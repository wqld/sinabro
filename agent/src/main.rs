pub mod context;
pub mod node_route;

use std::{
    env,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use anyhow::Error;
use ipnet::IpNet;

use context::Context;

macro_rules! run_command {
    ($command:expr $(, $args:expr)*) => {
        std::process::Command::new($command).args([$($args),*]).output()
            .expect("failed to run command")
    };
}

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

    let out = run_command!("apt", "update");

    match out.status.success() {
        true => Ok(()),
        _ => Err(Error::msg(String::from_utf8(out.stderr).unwrap())),
    }
}
