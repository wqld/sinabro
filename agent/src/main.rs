pub mod cni_config;
pub mod context;
pub mod node_route;

use std::{
    env,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use ipnet::IpNet;
use tracing::{debug, error, info};

use crate::{cni_config::CniConfig, context::Context};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Hello, world!");

    let context = Context::new().await?;

    let host_ip = env::var("HOST_IP").unwrap_or("172.18.0.2".to_owned());
    debug!("host ip: {}", host_ip);

    let node_routes = context.get_node_routes().await?;
    debug!("node routes: {:?}", node_routes);

    let host_route = node_routes
        .iter()
        .find(|node_route| node_route.ip == host_ip)
        .ok_or_else(|| anyhow::anyhow!("failed to find node route"))?;
    debug!("host route: {:?}", host_route);

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
        })?;
    let bridge_ip = IpNet::new(
        bridge_ip,
        host_route.pod_cidr.parse::<IpNet>()?.prefix_len(),
    )?;
    let bridge_ip = format!("{:?}", bridge_ip);
    debug!("bridge ip: {}", bridge_ip);

    let cluster_cidr = context.get_cluster_cidr().await?;
    debug!("cluster cidr: {}", cluster_cidr);

    let bridge_name = "cni0";

    // create and configure the bridge with the cni0 name
    run_command("ip", &["link", "add", bridge_name, "type", "bridge"])?;
    run_command("ip", &["link", "set", bridge_name, "up"])?;
    run_command(
        "ip",
        &["addr", "add", bridge_ip.as_str(), "dev", bridge_name],
    )?;

    // apply additional forwarding rules that will allow
    // to freely forward traffic inside the whole pod CIDR range
    run_command(
        "iptables",
        &[
            "-t",
            "filter",
            "-A",
            "FORWARD",
            "-s",
            cluster_cidr.as_str(),
            "-j",
            "ACCEPT",
        ],
    )?;
    run_command(
        "iptables",
        &[
            "-t",
            "filter",
            "-A",
            "FORWARD",
            "-d",
            cluster_cidr.as_str(),
            "-j",
            "ACCEPT",
        ],
    )?;

    // setup a network address translation (NAT)
    run_command(
        "iptables",
        &[
            "-t",
            "nat",
            "-A",
            "POSTROUTING",
            "-s",
            host_route.pod_cidr.as_str(),
            "!",
            "-o",
            bridge_name,
            "-j",
            "MASQUERADE",
        ],
    )?;

    // setup additional route rule
    node_routes
        .iter()
        .filter(|node_route| node_route.ip != host_ip)
        .try_for_each(|node_route| {
            run_command(
                "ip",
                &[
                    "route",
                    "add",
                    node_route.pod_cidr.as_str(),
                    "via",
                    node_route.ip.as_str(),
                    "dev",
                    "eth0", // TODO: need to retrieve the name of the interface on the host
                ],
            )
        })?;

    CniConfig::new(&cluster_cidr, &host_route.pod_cidr).write("/etc/cni/net.d/10-sinabro.conf")?;

    // wait forever
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }

    Ok(())
}

fn run_command(cmd: &str, args: &[&str]) -> anyhow::Result<()> {
    info!("running command: {} {}", cmd, args.join(" "));

    let out = std::process::Command::new(cmd)
        .args(args)
        .output()
        .expect("failed to run command");

    match out.status.success() {
        true => {}
        _ => error!("{}", String::from_utf8_lossy(&out.stderr)),
    }

    Ok(())
}
