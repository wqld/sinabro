mod bpf_loader;
mod context;
mod node_route;
mod server;

use std::env;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use std::sync::Arc;

use clap::Parser;
use ipnet::IpNet;
use log::{debug, info};
use sinabro_config::generate_mac_addr;
use sinabro_netlink::netlink::Netlink;
use sinabro_netlink::route::addr::AddressBuilder;
use sinabro_netlink::route::link::{Kind, Link, LinkAttrs, VxlanAttrs};
use sinabro_netlink::route::routing::{RoutingBuilder, Via};
use tokio::sync::Notify;
use tracing::Level;

use crate::context::Context;

#[derive(Debug, Parser)]
struct Opt {
    #[clap(short, long, default_value = "eth0")]
    iface: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    sinabro_config::setup_tracing_to_stdout(Level::DEBUG);

    let context = Context::new().await?;

    let host_ip = env::var("HOST_IP").expect("HOST_IP is not set");
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

    let mut netlink = Netlink::new();

    let bridge = Kind::new_bridge(bridge_name);
    let bridge = netlink.ensure_link(&bridge)?;

    let address = AddressBuilder::default()
        .ip(bridge_ip.as_str().parse::<IpNet>()?)
        .build()?;

    if let Err(e) = netlink.addr_add(&bridge, &address) {
        if e.to_string().contains("File exists") {
            info!("cni0 interface already has an ip address");
        } else {
            return Err(e);
        }
    }

    let eth0_attrs = LinkAttrs::new("eth0");
    let eth0 = netlink.link_get(&eth0_attrs)?;
    netlink.link_up(&eth0)?;

    node_routes
        .iter()
        .filter(|node_route| node_route.ip != host_ip)
        .try_for_each(|node_route| {
            let route = RoutingBuilder::default()
                .oif_index(eth0.attrs().index)
                .dst(Some(node_route.pod_cidr.parse().unwrap()))
                .via(Some(Via::new(&node_route.ip).unwrap()))
                .build()?;

            if let Err(e) = netlink.route_add(&route) {
                if e.to_string().contains("File exists") {
                    info!("route already exists");
                    Ok(())
                } else {
                    Err(e)
                }
            } else {
                Ok(())
            }
        })?;

    let vxlan_mac = generate_mac_addr()?;
    let vxlan_dev = Kind::Vxlan {
        attrs: LinkAttrs {
            name: "sinabro_vxlan".to_string(),
            mtu: 1500,
            hw_addr: vxlan_mac,
            ..Default::default()
        },
        vxlan_attrs: VxlanAttrs {
            flow_based: true,
            port: Some(8472),
            ..Default::default()
        },
    };
    let _vxlan_dev = netlink.ensure_link(&vxlan_dev)?;

    sinabro_config::Config::new(&cluster_cidr, &host_route.pod_cidr)
        .write("/etc/cni/net.d/10-sinabro.conf")?;

    let pod_cidr = host_route.pod_cidr.clone();
    let store_path = "/var/lib/sinabro/ip_store"; // TODO: make this configurable
    let shutdown = Arc::new(Notify::new());
    let node_ips: Vec<String> = node_routes
        .iter()
        .map(|node_route| node_route.ip.clone())
        .collect();

    let bpf_loader = bpf_loader::BpfLoader::new(&opt.iface);
    bpf_loader
        .load(
            &host_ip,
            &cluster_cidr,
            &pod_cidr,
            &node_ips,
            store_path,
            shutdown,
        )
        .await?;

    Ok(())
}
