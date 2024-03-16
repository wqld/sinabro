use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    ops::{Deref, DerefMut},
};

use anyhow::{anyhow, Result};
use ipnet::IpNet;
use rsln::types::{
    addr::AddressBuilder,
    link::{Kind, Link, LinkAttrs, VxlanAttrs},
    neigh::NeighborBuilder,
    routing::{RoutingBuilder, Via},
};
use sinabro_config::generate_mac;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{kube::Context, node_route::NodeRoute};

const RTNH_F_ONLINK: u32 = 0x4;
const BRIDGE_NAME: &str = "cni0";

#[derive(Default)]
pub struct Netlink<'a> {
    pub netlink: rsln::netlink::Netlink,
    pub host_ip: Option<&'a str>,
    pub pod_cidr: Option<&'a IpNet>,
    pub node_routes: Option<&'a [NodeRoute]>,
}

impl<'a> Deref for Netlink<'a> {
    type Target = rsln::netlink::Netlink;

    fn deref(&self) -> &Self::Target {
        &self.netlink
    }
}

impl<'a> DerefMut for Netlink<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.netlink
    }
}

impl<'a> Netlink<'a> {
    pub fn new() -> Self {
        Self {
            netlink: rsln::netlink::Netlink::new(),
            ..Default::default()
        }
    }

    pub fn init(host_ip: &'a str, pod_cidr: &'a IpNet, node_routes: &'a [NodeRoute]) -> Self {
        Self {
            netlink: rsln::netlink::Netlink::new(),
            host_ip: Some(host_ip),
            pod_cidr: Some(pod_cidr),
            node_routes: Some(node_routes),
        }
    }

    pub fn setup_bridge(&mut self) -> Result<i32> {
        let pod_cidr = self.pod_cidr.ok_or(anyhow!("pod_cidr is not set"))?;
        let ip_addr = Self::get_ip_addr(pod_cidr);
        let bridge = self.ensure_link(&Kind::new_bridge(BRIDGE_NAME))?;
        let address = AddressBuilder::default()
            .ip(IpNet::new(ip_addr, pod_cidr.prefix_len())?)
            .build()?;

        if let Err(e) = self.addr_add(&bridge, &address) {
            if e.to_string().contains("File exists") {
                info!("cni0 interface already has an ip address");
            } else {
                return Err(e);
            }
        }

        Ok(bridge.attrs().index)
    }

    pub fn setup_vxlan(&mut self) -> Result<i32> {
        let host_ip = self.host_ip.ok_or(anyhow!("host_ip is not set"))?;
        let pod_cidr = self.pod_cidr.ok_or(anyhow!("pod_cidr is not set"))?;

        let eth0_attrs = LinkAttrs::new("eth0");
        let eth0 = self.link_get(&eth0_attrs)?;
        let vtep_index = eth0.attrs().index as u32;
        self.link_up(&eth0)?;

        let vxlan_mac = generate_mac()?;
        let host_ip_bytes = match host_ip.parse::<IpAddr>()? {
            IpAddr::V4(ip) => ip.octets().to_vec(),
            IpAddr::V6(ip) => ip.octets().to_vec(),
        };

        let vxlan = Kind::Vxlan {
            attrs: LinkAttrs {
                name: "sinabro_vxlan".into(),
                mtu: 1450,
                hw_addr: vxlan_mac,
                ..Default::default()
            },
            vxlan_attrs: VxlanAttrs {
                id: 1,
                vtep_index: Some(vtep_index),
                src_addr: Some(host_ip_bytes),
                port: Some(8472),
                ..Default::default()
            },
        };

        let vxlan = self.ensure_link(&vxlan)?;
        let vxlan_addr = IpNet::new(pod_cidr.addr(), 32)?;
        let vxlan_addr = AddressBuilder::default().ip(vxlan_addr).build()?;

        if let Err(e) = self.addr_add(&vxlan, &vxlan_addr) {
            if e.to_string().contains("File exists") {
                info!("vxlan interface already has an ip address");
            } else {
                return Err(e);
            }
        }

        Ok(vxlan.attrs().index)
    }

    pub fn initialize_overlay(&mut self, vxlan_index: i32) -> Result<()> {
        let host_ip = self.host_ip.ok_or(anyhow!("host_ip is not set"))?;

        if let Some(node_routes) = self.node_routes {
            node_routes
                .iter()
                .filter(|node_route| node_route.ip != host_ip)
                .for_each(|node_route| {
                    let node_route_pod_cidr = node_route.pod_cidr.clone();
                    let node_route_ip = node_route.ip.clone();

                    tokio::spawn(async move {
                        Self::setup_route_and_neighbors(
                            &node_route_ip,
                            &node_route_pod_cidr,
                            vxlan_index,
                        )
                        .await
                    });
                });
        }

        Ok(())
    }

    async fn setup_route_and_neighbors(
        node_ip: &str,
        pod_cidr: &str,
        vxlan_index: i32,
    ) -> Result<()> {
        let mut netlink = Netlink::new();
        let token = CancellationToken::new();
        let context = Context::new(token).await?;
        let pod_cidr_ip_net = pod_cidr.parse::<IpNet>()?;

        let route = RoutingBuilder::default()
            .oif_index(vxlan_index)
            .dst(Some(pod_cidr_ip_net))
            .via(Some(Via::new(&pod_cidr_ip_net.addr().to_string())?))
            .flags(RTNH_F_ONLINK)
            .build()?;

        if let Err(e) = netlink.route_add(&route) {
            if e.to_string().contains("File exists") {
                info!("route already exists");
            } else {
                return Err(e);
            }
        }

        let vxlan_mac = context.get_vxlan_mac_address(node_ip).await?;

        let neigh = NeighborBuilder::default()
            .link_index(vxlan_index as u32)
            .state(libc::NUD_PERMANENT)
            .neigh_type(libc::RTN_UNICAST)
            .ip_addr(Some(pod_cidr_ip_net.network()))
            .mac_addr(Some(vxlan_mac.clone()))
            .build()?;

        if let Err(e) = netlink.neigh_set(&neigh) {
            if e.to_string().contains("File exists") {
                info!("neighbor already exists");
            } else {
                error!("error: {:?}", e);
                return Err(e);
            }
        }

        let fdb = NeighborBuilder::default()
            .link_index(vxlan_index as u32)
            .state(libc::NUD_PERMANENT)
            .family(Some(libc::AF_BRIDGE as u8))
            .flags(libc::NTF_SELF)
            .ip_addr(Some(node_ip.parse::<IpAddr>()?))
            .mac_addr(Some(vxlan_mac))
            .build()?;

        if let Err(e) = netlink.neigh_set(&fdb) {
            if e.to_string().contains("File exists") {
                info!("fdb already exists");
            } else {
                error!("error: {:?}", e);
                return Err(e);
            }
        }

        info!("completed setting up routes and neighbors for {}", node_ip);
        Ok(())
    }

    fn get_ip_addr(ip_net: &IpNet) -> IpAddr {
        match ip_net {
            IpNet::V4(v4) => {
                let net = u32::from(v4.network()) + 1;
                IpAddr::V4(Ipv4Addr::from(net))
            }
            IpNet::V6(v6) => {
                let net = u128::from(v6.network()) + 1;
                IpAddr::V6(Ipv6Addr::from(net))
            }
        }
    }
}
