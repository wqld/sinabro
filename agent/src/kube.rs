use std::fmt::Debug;

use anyhow::{anyhow, bail, Result};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::{ConfigMap, Node, Pod, Service};
use kube::{
    api::{AttachParams, AttachedProcess, ListParams, WatchEvent, WatchParams},
    runtime::{watcher, WatchStreamExt},
    Api, ResourceExt,
};
use sinabro_config::parse_mac;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::node_route::NodeRoute;

pub struct Context {
    client: kube::Client,
    token: CancellationToken,
}

impl Context {
    pub async fn new(token: CancellationToken) -> Result<Self> {
        let client = kube::Client::try_default().await?;
        Ok(Self { client, token })
    }

    pub async fn get_cluster_cidr(&self) -> Result<String> {
        Api::<ConfigMap>::namespaced(self.client.clone(), "kube-system")
            .get("kube-proxy")
            .await?
            .data
            .and_then(|data| data.get("config.conf").cloned())
            .and_then(|conf| serde_yaml::from_str::<serde_yaml::Value>(&conf).ok())
            .and_then(|yaml| yaml["clusterCIDR"].as_str().map(ToOwned::to_owned))
            .ok_or_else(|| anyhow!("failed to get cluster cidr"))
    }

    pub async fn get_node_routes(&self) -> Result<Vec<NodeRoute>> {
        Ok(Api::<Node>::all(self.client.clone())
            .list(&Default::default())
            .await?
            .items
            .into_iter()
            .map(NodeRoute::from)
            .collect())
    }

    pub async fn get_vxlan_mac_address(&self, node_ip: &str) -> Result<Vec<u8>> {
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), "kube-system");
        let lp = ListParams::default().labels("name=agent");

        for p in pods.list(&lp).await? {
            let pod_name = p.metadata.name.unwrap_or_default();
            Self::watch_pod_until_running(&pods, &pod_name).await?;

            if p.status
                .and_then(|status| status.host_ip)
                .filter(|host_ip| host_ip == node_ip)
                .is_some()
            {
                let command = vec!["ip", "link", "show", "sinabro_vxlan"];
                return Self::exec_command_in_pod(&pods, &pod_name, command)
                    .await?
                    .lines()
                    .find_map(|line| {
                        if line.contains("link/ether") {
                            line.split_whitespace().nth(1).map(parse_mac)
                        } else {
                            None
                        }
                    })
                    .ok_or(anyhow!("failed to get vxlan mac address"))?;
            }
        }

        bail!("failed to get vxlan mac address")
    }

    pub async fn watch_service_resource(&self) -> Result<()> {
        let services: Api<Service> = Api::all(self.client.clone());
        let watch_future = watcher(services, watcher::Config::default())
            .default_backoff()
            .try_for_each(|s| async move {
                info!("Service event: {:?}", s);
                Ok(())
            });

        tokio::select! {
            _ = watch_future => {},
            _ = self.token.cancelled() => {}
        }

        Ok(())
    }

    async fn watch_pod_until_running(pods: &Api<Pod>, name: &str) -> Result<()> {
        let wp = WatchParams::default()
            .fields(&format!("metadata.name={}", name))
            .timeout(10);
        let mut stream = pods.watch(&wp, "0").await?.boxed();

        while let Some(status) = stream.try_next().await? {
            match status {
                WatchEvent::Added(o) => {
                    info!("Pod {} has been added", o.name_any());
                }
                WatchEvent::Modified(o) => {
                    let s = o.status.as_ref().expect("status exists on pod");
                    if s.phase.as_deref() == Some("Running") {
                        info!("Ready to attach to {}", o.name_any());
                        break;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn exec_command_in_pod<I: Debug, T>(
        pods: &Api<Pod>,
        name: &str,
        command: I,
    ) -> Result<String>
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        let attached = pods
            .exec(name, command, &AttachParams::default().stderr(false))
            .await?;

        Self::get_output(attached).await
    }

    async fn get_output(mut attached: AttachedProcess) -> Result<String> {
        let stdout = tokio_util::io::ReaderStream::new(
            attached.stdout().ok_or(anyhow!("stdout not found"))?,
        );
        let out = stdout
            .filter_map(|r| async { r.ok().and_then(|v| String::from_utf8(v.to_vec()).ok()) })
            .collect::<Vec<_>>()
            .await
            .join("");

        attached.join().await?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use futures::pin_mut;
    use http::{Request, Response};
    use hyper::Body;
    use kube::core::ObjectList;
    use tower_test::mock;

    use super::*;

    #[tokio::test]
    async fn test_get_cluster_cidr() {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            let (request, send) = handle.next_request().await.expect("service not called");
            assert_eq!(request.method(), &http::Method::GET);
            assert_eq!(
                request.uri().path(),
                "/api/v1/namespaces/kube-system/configmaps/kube-proxy"
            );

            let config_map: ConfigMap = serde_json::from_value(serde_json::json!({
                "apiVersion": "v1",
                "kind": "ConfigMap",
                "metadata": {
                    "labels": {
                        "app": "kube-proxy"
                    },
                    "name": "kube-proxy",
                    "namespace": "kube-system",
                },
                "data": {
                    "config.conf": "apiVersion: kubeproxy.config.k8s.io/v1alpha1\nbindAddress: 0.0.0.0\nbindAddressHardFail: false\nclientConnection:\n  acceptContentTypes: \"\"\n  burst: 0\n  contentType: \"\"\n  kubeconfig: /var/lib/kube-proxy/kubeconfig.conf\n  qps: 0\nclusterCIDR: 10.244.0.0/16\nconfigSyncPeriod: 0s\nconntrack:\n  maxPerCore: 0\n  min: null\n  tcpCloseWaitTimeout: null\n  tcpEstablishedTimeout: null\ndetectLocal:\n  bridgeInterface: \"\"\n  interfaceNamePrefix: \"\"\ndetectLocalMode: \"\"\nenableProfiling: false\nhealthzBindAddress: \"\"\nhostnameOverride: \"\"\niptables:\n  localhostNodePorts: null\n  masqueradeAll: false\n  masqueradeBit: null\n  minSyncPeriod: 1s\n  syncPeriod: 0s\nipvs:\n  excludeCIDRs: null\n  minSyncPeriod: 0s\n  scheduler: \"\"\n  strictARP: false\n  syncPeriod: 0s\n  tcpFinTimeout: 0s\n  tcpTimeout: 0s\n  udpTimeout: 0s\nkind: KubeProxyConfiguration\nmetricsBindAddress: \"\"\nmode: iptables\nnodePortAddresses: null\noomScoreAdj: null\nportRange: \"\"\nshowHiddenMetricsForVersion: \"\"\nwinkernel:\n  enableDSR: false\n  forwardHealthCheckVip: false\n  networkName: \"\"\n  rootHnsEndpointName: \"\"\n  sourceVip: \"\"",
                    "kubeconfig.conf": "apiVersion: v1\nkind: Config\nclusters:\n- cluster:\n    certificate-authority: /var/run/secrets/kubernetes.io/serviceaccount/ca.crt\n    server: https://kind-control-plane:6443\n  name: default\ncontexts:\n- context:\n    cluster: default\n    namespace: default\n    user: default\n  name: default\ncurrent-context: default\nusers:\n- name: default\n  user:\n    tokenFile: /var/run/secrets/kubernetes.io/serviceaccount/token"
                }
            }))
            .unwrap();

            send.send_response(
                Response::builder()
                    .body(Body::from(serde_json::to_vec(&config_map).unwrap()))
                    .unwrap(),
            );
        });

        let client = kube::Client::new(mock_service, "test-namespace");
        let token = CancellationToken::new();
        let context = Context { client, token };
        let cluster_cidr = context.get_cluster_cidr().await.unwrap();
        assert_eq!(cluster_cidr, "10.244.0.0/16");

        spawned.await.unwrap();
    }

    #[tokio::test]
    async fn test_get_node_routes() {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            let (request, send) = handle.next_request().await.expect("service not called");
            assert_eq!(request.method(), &http::Method::GET);
            assert_eq!(request.uri().path(), "/api/v1/nodes");

            let nodes: ObjectList<Node> = serde_json::from_value(serde_json::json!({
                "apiVersion": "v1",
                "items": [
                  {
                    "apiVersion": "v1",
                    "kind": "Node",
                    "metadata": {
                      "labels": {
                        "kubernetes.io/hostname": "kind-control-plane",
                      },
                      "name": "kind-control-plane",
                    },
                    "spec": {
                      "podCIDR": "10.244.0.0/24",
                      "podCIDRs": [
                        "10.244.0.0/24"
                      ]
                    },
                    "status": {
                      "addresses": [
                        {
                          "address": "172.18.0.3",
                          "type": "InternalIP"
                        },
                        {
                          "address": "kind-control-plane",
                          "type": "Hostname"
                        }
                      ]
                    }
                  },
                  {
                    "apiVersion": "v1",
                    "kind": "Node",
                    "metadata": {
                      "labels": {
                        "kubernetes.io/hostname": "kind-worker",
                      },
                      "name": "kind-worker",
                    },
                    "spec": {
                      "podCIDR": "10.244.1.0/24",
                      "podCIDRs": [
                        "10.244.1.0/24"
                      ]
                    },
                    "status": {
                      "addresses": [
                        {
                          "address": "172.18.0.2",
                          "type": "InternalIP"
                        },
                        {
                          "address": "kind-worker",
                          "type": "Hostname"
                        }
                      ]
                    }
                  }
                ],
                "kind": "List",
                "metadata": {
                  "resourceVersion": ""
                }
            }))
            .unwrap();

            send.send_response(
                Response::builder()
                    .body(Body::from(serde_json::to_vec(&nodes).unwrap()))
                    .unwrap(),
            );
        });

        let client = kube::Client::new(mock_service, "test-namespace");
        let token = CancellationToken::new();
        let context = Context { client, token };
        let node_routes = context.get_node_routes().await.unwrap();
        assert_eq!(node_routes.len(), 2);
        assert_eq!(node_routes[0].ip, "172.18.0.3");
        assert_eq!(node_routes[0].pod_cidr, "10.244.0.0/24");
        assert_eq!(node_routes[1].ip, "172.18.0.2");
        assert_eq!(node_routes[1].pod_cidr, "10.244.1.0/24");

        spawned.await.unwrap();
    }
}
