use k8s_openapi::api::core::v1::{ConfigMap, Node};

use crate::node_route::NodeRoute;

pub struct Context {
    client: kube::Client,
}

impl Context {
    pub async fn new() -> anyhow::Result<Self> {
        let client = kube::Client::try_default().await?;
        Ok(Self { client })
    }

    pub async fn get_cluster_cidr(&self) -> anyhow::Result<String> {
        kube::Api::<ConfigMap>::namespaced(self.client.clone(), "kube-system")
            .get("kube-proxy")
            .await?
            .data
            .and_then(|data| data.get("config.conf").cloned())
            .and_then(|conf| serde_yaml::from_str::<serde_yaml::Value>(&conf).ok())
            .and_then(|yaml| yaml["clusterCIDR"].as_str().map(ToOwned::to_owned))
            .ok_or_else(|| anyhow::anyhow!("failed to get cluster cidr"))
    }

    pub async fn get_node_routes(&self) -> anyhow::Result<Vec<NodeRoute>> {
        Ok(kube::Api::<Node>::all(self.client.clone())
            .list(&Default::default())
            .await?
            .items
            .into_iter()
            .map(NodeRoute::from)
            .collect())
    }
}
