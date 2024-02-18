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

const RTNH_F_ONLINK: u32 = 0x4;

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

    let pod_cidr_ip_net = host_route.pod_cidr.parse::<IpNet>()?;
    let bridge_ip = match pod_cidr_ip_net {
        IpNet::V4(v4) => {
            let net = u32::from(v4.network()) + 1;
            IpAddr::V4(Ipv4Addr::from(net))
        }
        IpNet::V6(v6) => {
            let net = u128::from(v6.network()) + 1;
            IpAddr::V6(Ipv6Addr::from(net))
        }
    };
    let bridge_ip = IpNet::new(bridge_ip, pod_cidr_ip_net.prefix_len())?;
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

    let host_ip_vec = match host_ip.parse::<IpAddr>()? {
        IpAddr::V4(ip) => ip.octets().to_vec(),
        IpAddr::V6(ip) => ip.octets().to_vec(),
    };
    let vxlan_mac = generate_mac_addr()?;
    let vxlan = Kind::Vxlan {
        attrs: LinkAttrs {
            name: "sinabro_vxlan".to_string(),
            mtu: 1450,
            hw_addr: vxlan_mac,
            ..Default::default()
        },
        vxlan_attrs: VxlanAttrs {
            id: 1,
            vtep_index: Some(eth0.attrs().index as u32),
            src_addr: Some(host_ip_vec),
            port: Some(8472),
            ..Default::default()
        },
    };
    let vxlan = netlink.ensure_link(&vxlan)?;

    let vxlan_ip = IpNet::new(pod_cidr_ip_net.addr(), 32)?;
    let vxlan_addr = AddressBuilder::default().ip(vxlan_ip).build()?;

    netlink.addr_add(&vxlan, &vxlan_addr)?;

    node_routes
        .iter()
        .filter(|node_route| node_route.ip != host_ip)
        .try_for_each(|node_route| {
            let pod_cidr_ip_net = node_route.pod_cidr.parse::<IpNet>()?;
            let route = RoutingBuilder::default()
                .oif_index(vxlan.attrs().index)
                .dst(Some(pod_cidr_ip_net))
                .via(Some(Via::new(&pod_cidr_ip_net.addr().to_string())?))
                .flags(RTNH_F_ONLINK)
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
