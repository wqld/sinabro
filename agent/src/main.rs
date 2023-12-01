// pod cidr -> from node's pod cidr
// node is -> from node
// whole pod cidr range ? -> cluster-info dump cluster-cidr
// bridge ip -> pod cidr + 1

use std::env;

use k8s_openapi::api::core::v1::{ConfigMap, Node};
use kube::{Api, Client};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");

    // let host_ip = env::var("HOST_IP")?;
    // println!("host ip: {}", host_ip);

    let context = Context::new().await?;

    let client = Client::try_default().await?;

    let node_api: Api<Node> = Api::all(client.clone());

    // get all nodes
    let nodes = node_api.list(&Default::default()).await?;

    // get node's pod cidr & node ip from nodes
    for node in nodes.items {
        let node = node.clone();
        let node_name = node.metadata.name.unwrap();
        let node_ip = &node.status.unwrap().addresses.unwrap()[0].address;
        let node_pod_cidr = node.spec.unwrap().pod_cidr.unwrap();
        println!("node name: {}", node_name);
        println!("node ip: {}", node_ip);
        println!("node pod cidr: {}", node_pod_cidr);
    }

    // get cluster cidr
    let cluster_cidr = context.get_cluster_cidr().await?;
    println!("cluster cidr: {}", cluster_cidr);

    Ok(())
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
        let config_map_api: Api<ConfigMap> = Api::namespaced(self.client.clone(), "kube-system");

        config_map_api
            .get("kube-proxy")
            .await?
            .data
            .and_then(|data| data.get("config.conf").cloned())
            .and_then(|conf| serde_yaml::from_str::<serde_yaml::Value>(&conf).ok())
            .and_then(|yaml| yaml["clusterCIDR"].as_str().map(ToOwned::to_owned))
            .ok_or_else(|| anyhow::anyhow!("failed to get cluster cidr"))
    }
}
