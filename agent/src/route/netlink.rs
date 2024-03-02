use std::{
    net::IpAddr,
    ops::{Deref, DerefMut},
};

use anyhow::Result;
use ipnet::IpNet;
use sinabro_config::generate_mac;
use sinabro_netlink::route::{
    addr::AddressBuilder,
    link::{Kind, Link, LinkAttrs, VxlanAttrs},
    neigh::NeighborBuilder,
    routing::{RoutingBuilder, Via},
};
use tracing::{error, info};

use crate::{context::Context, node_route::NodeRoute};

const RTNH_F_ONLINK: u32 = 0x4;

pub struct Netlink {
    pub netlink: sinabro_netlink::netlink::Netlink,
}

impl Deref for Netlink {
    type Target = sinabro_netlink::netlink::Netlink;

    fn deref(&self) -> &Self::Target {
        &self.netlink
    }
}

impl DerefMut for Netlink {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.netlink
    }
}

impl Netlink {
    pub fn new() -> Self {
        Netlink {
            netlink: sinabro_netlink::netlink::Netlink::new(),
        }
    }

    pub fn setup_vxlan_device(
        &mut self,
        host_ip: &str,
        vtep_index: u32,
        pod_cidr: &IpNet,
    ) -> Result<i32> {
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

    pub fn initialize_overlay(
        &mut self,
        vxlan_index: i32,
        host_ip: &str,
        node_routes: &[NodeRoute],
    ) -> Result<()> {
        node_routes
            .iter()
            .filter(|node_route| node_route.ip != host_ip)
            .for_each(|node_route| {
                let node_route_pod_cidr = node_route.pod_cidr.clone();
                let node_route_ip = node_route.ip.clone();

                tokio::spawn(async move {
                    Self::setup_route_and_neighbors(
                        node_route_ip.as_str(),
                        node_route_pod_cidr.as_str(),
                        vxlan_index,
                    )
                    .await
                });
            });

        Ok(())
    }

    async fn setup_route_and_neighbors(
        node_ip: &str,
        pod_cidr: &str,
        vxlan_index: i32,
    ) -> Result<()> {
        let mut netlink = Netlink::new();
        let context = Context::new().await?;
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
}
