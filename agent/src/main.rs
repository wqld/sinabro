mod bpf_loader;
mod context;
mod node_route;
mod route;
mod server;

use std::env;
use std::sync::Arc;

use anyhow::Result;
use aya_log::BpfLogger;
use bpf_loader::BpfLoader;
use clap::Parser;
use ipnet::IpNet;
use node_route::NodeRoute;
use server::api_server;
use sinabro_config::{setup_tracing_to_stdout, Config};
use tokio::sync::Notify;
use tracing::Level;

use crate::context::Context;
use crate::route::netlink::Netlink;

#[derive(Debug, Parser)]
struct Opt {
    #[clap(short, long, default_value = "eth0")]
    iface: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_tracing_to_stdout(Level::DEBUG);

    let opt = Opt::parse();
    let context = Context::new().await?;

    let node_routes = context.get_node_routes().await?;
    let cluster_cidr = context.get_cluster_cidr().await?;
    let host_ip = get_host_ip()?;
    let host_route = find_host_route(&node_routes, &host_ip)?;

    setup_network(&host_ip, host_route, &node_routes)?;
    setup_cni_config(&cluster_cidr, &host_route.pod_cidr)?;

    let mut bpf_loader = BpfLoader::load(&opt.iface)?;
    let node_ips = get_node_ips(&node_routes);

    BpfLogger::init(&mut bpf_loader.bpf)?;

    bpf_loader
        .attach(&host_ip, &cluster_cidr, &node_ips)
        .await?;

    start_api_server(&host_route.pod_cidr).await?;

    Ok(())
}

fn get_host_ip() -> Result<String> {
    env::var("HOST_IP").map_err(|_| anyhow::anyhow!("HOST_IP is not set"))
}

fn find_host_route<'a>(node_routes: &'a [NodeRoute], host_ip: &str) -> Result<&'a NodeRoute> {
    node_routes
        .iter()
        .find(|node_route| node_route.ip == host_ip)
        .ok_or_else(|| anyhow::anyhow!("failed to find node route"))
}

fn setup_network(host_ip: &str, host_route: &NodeRoute, node_routes: &[NodeRoute]) -> Result<()> {
    let pod_cidr = host_route.pod_cidr.parse::<IpNet>()?;
    let mut netlink = Netlink::init(host_ip, &pod_cidr, node_routes);
    let _ = netlink.setup_bridge()?;
    let vxlan_index = netlink.setup_vxlan()?;
    netlink.initialize_overlay(vxlan_index)?;

    Ok(())
}

fn setup_cni_config(cluster_cidr: &str, pod_cidr: &str) -> Result<()> {
    Config::new(cluster_cidr, pod_cidr).write("/etc/cni/net.d/10-sinabro.conf")?;
    Ok(())
}

fn get_node_ips(node_routes: &[NodeRoute]) -> Vec<String> {
    node_routes
        .iter()
        .map(|node_route| node_route.ip.clone())
        .collect()
}

async fn start_api_server(pod_cidr: &str) -> Result<()> {
    let store_path = "/var/lib/sinabro/ip_store"; // TODO: make this configurable
    let shutdown = Arc::new(Notify::new());

    api_server::start(pod_cidr, store_path, shutdown)
        .await
        .unwrap();

    Ok(())
}
