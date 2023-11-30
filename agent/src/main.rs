// pod cidr -> from node's pod cidr
// node is -> from node
// whole pod cidr range ? -> cluster-info dump cluster-cidr
// bridge ip -> pod cidr + 1

use k8s_openapi::api::core::v1::Node;
use kube::{Api, Client};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");

    let client = Client::try_default().await?;

    let node_api: Api<Node> = Api::all(client);

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
        // get cluster cidr
        let cluster_cidr = node_pod_cidr.split(".").collect::<Vec<&str>>()[0..3].join(".");
        println!("cluster cidr: {}", cluster_cidr);
        // get bridge ip
        let bridge_ip = format!("{}.1", cluster_cidr);
        println!("bridge ip: {}", bridge_ip);
    }

    Ok(())
}
