// pod cidr -> from node's pod cidr
// node is -> from node
// whole pod cidr range ? -> cluster-info dump cluster-cidr
// bridge ip -> pod cidr + 1

use std::{
    env,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use ipnet::{IpAdd, IpNet, Ipv4Net};
use k8s_openapi::api::core::v1::{ConfigMap, Node};
use kube::{Api, Client};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");

    let host_ip = env::var("HOST_IP").unwrap_or("172.18.0.2".to_owned());
    println!("host ip: {}", host_ip);

    let context = Context::new().await?;

    let node_routes = context.get_node_routes().await?;
    println!("node routes: {:?}", node_routes);

    let host_route = node_routes
        .iter()
        .find(|node_route| node_route.ip == host_ip)
        .ok_or_else(|| anyhow::anyhow!("failed to find node route"))?;
    println!("host route: {:?}", host_route);

    let bridge_ip = host_route
        .pod_cidr
        .parse::<IpNet>()
        .map(|ipnet| match ipnet {
            IpNet::V4(v4) => {
                let net = u32::from(v4.network()) + 1;
                IpAddr::V4(Ipv4Addr::from(net))
            }
            IpNet::V6(v6) => {
                let net = u128::from(v6.network()) + 1;
                IpAddr::V6(Ipv6Addr::from(net))
            }
        });
    println!("bridge ip: {:?}", bridge_ip?);

    let cluster_cidr = context.get_cluster_cidr().await?;
    println!("cluster cidr: {}", cluster_cidr);

    Ok(())
}

#[derive(Debug)]
struct NodeRoute {
    ip: String,
    pod_cidr: String,
}

impl From<Node> for NodeRoute {
    fn from(node: Node) -> Self {
        let ip = node
            .status
            .and_then(|status| status.addresses)
            .and_then(|addresses| addresses.get(0).cloned())
            .map(|address| address.address)
            .unwrap_or_default();
        let pod_cidr = node.spec.and_then(|spec| spec.pod_cidr).unwrap_or_default();

        Self { ip, pod_cidr }
    }
}

struct Context {
    client: Client,
}

impl Context {
    async fn new() -> anyhow::Result<Self> {
        let client = Client::try_default().await?;
        Ok(Self { client })
    }

    async fn get_cluster_cidr(&self) -> anyhow::Result<String> {
        Api::<ConfigMap>::namespaced(self.client.clone(), "kube-system")
            .get("kube-proxy")
            .await?
            .data
            .and_then(|data| data.get("config.conf").cloned())
            .and_then(|conf| serde_yaml::from_str::<serde_yaml::Value>(&conf).ok())
            .and_then(|yaml| yaml["clusterCIDR"].as_str().map(ToOwned::to_owned))
            .ok_or_else(|| anyhow::anyhow!("failed to get cluster cidr"))
    }

    async fn get_node_routes(&self) -> anyhow::Result<Vec<NodeRoute>> {
        Ok(Api::<Node>::all(self.client.clone())
            .list(&Default::default())
            .await?
            .items
            .into_iter()
            .map(NodeRoute::from)
            .collect())
    }
}
