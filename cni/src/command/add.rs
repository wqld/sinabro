use std::{env, fs::File, net::IpAddr, os::fd::AsRawFd};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ipnet::IpNet;
use nix::sched::{setns, CloneFlags};
use rand::Rng;
use serde::Serialize;
use sinabro_config::Config;
use sinabro_netlink::{
    netlink::Netlink,
    route::{
        addr::AddressBuilder,
        link::{Kind, LinkAttrs},
        routing::RoutingBuilder,
    },
};
use tokio::task::spawn_blocking;
use tracing::info;

use crate::command::generate_mac_addr;

use super::CniCommand;

pub struct AddCommand;

#[async_trait]
impl CniCommand for AddCommand {
    async fn run(&self, cni_config: &Config) -> Result<()> {
        let netns = env::var("CNI_NETNS")?;
        let cni_if_name = env::var("CNI_IFNAME")?;
        let container_ip = Self::request_container_ip().await?;
        let subnet_mask_size = cni_config.subnet.split('/').last().unwrap();
        let container_addr = format!("{}/{}", container_ip, subnet_mask_size);

        let netns_file = File::open(&netns)?;
        let netns_fd = netns_file.as_raw_fd();

        let veth_suffix = Self::generate_veth_suffix();
        let veth_name = format!("veth{}", veth_suffix);
        let peer_name = format!("peer{}", veth_suffix);

        let mut netlink = Netlink::new();

        let cni0 = netlink.link_get(&LinkAttrs::new("cni0"))?;

        let mut veth_attr = LinkAttrs::new(&veth_name);
        veth_attr.mtu = 1500;
        veth_attr.tx_queue_len = 1000;
        veth_attr.hw_addr = generate_mac_addr()?;

        let veth = Kind::Veth {
            attrs: veth_attr.clone(),
            peer_name: peer_name.clone(),
            peer_hw_addr: Some(generate_mac_addr()?),
            peer_ns: None,
        };

        netlink.link_add(&veth)?;

        let veth = netlink.link_get(&veth_attr)?;
        let peer = netlink.link_get(&LinkAttrs::new(&peer_name))?;

        netlink.link_up(&veth)?;
        netlink.link_set_master(&veth, cni0.attrs().index)?;
        netlink.link_set_ns(&peer, netns_fd)?;

        let subnet = cni_config.subnet.parse::<IpNet>()?;
        let bridge_ip = subnet
            .hosts()
            .next()
            .map(|ip| ip.to_string())
            .ok_or_else(|| anyhow!("failed to get bridge ip"))?;

        let container_addr_clone = container_addr.clone();
        let bridge_ip_clone = bridge_ip.clone();

        let mac_addr = spawn_blocking(move || -> Result<String> {
            setns(netns_file, CloneFlags::CLONE_NEWNET)?;

            let mut netlink = Netlink::new();
            let link = netlink.link_get(&LinkAttrs::new(&peer_name))?;
            netlink.link_set_name(&link, &cni_if_name)?;
            netlink.link_up(&link)?;

            let container_addr = AddressBuilder::default()
                .ip(container_addr_clone.parse::<IpNet>()?)
                .build()?;

            if let Err(e) = netlink.addr_add(&link, &container_addr) {
                if e.to_string().contains("File exists") {
                    info!("eth0 interface already has an ip address");
                } else {
                    return Err(e);
                }
            }

            let route = RoutingBuilder::default()
                .oif_index(link.attrs().index)
                .gw(Some(bridge_ip_clone.parse::<IpAddr>()?))
                .build()?;

            if let Err(e) = netlink.route_add(&route) {
                if e.to_string().contains("File exists") {
                    info!("route already exists");
                } else {
                    return Err(e);
                }
            }

            Ok(link
                .attrs()
                .hw_addr
                .iter()
                .map(|byte| format!("{:02x}", byte))
                .collect::<Vec<String>>()
                .join(":"))
        })
        .await??;

        Self::print_result(&mac_addr, &netns, &container_addr, &bridge_ip);
        Ok(())
    }
}

impl AddCommand {
    async fn request_container_ip() -> Result<String> {
        let res = reqwest::get("http://localhost:3000/ipam/ip").await?;
        Ok(res.text().await?)
    }

    fn generate_veth_suffix() -> String {
        let mut rng = rand::thread_rng();
        let charset: &[u8] = b"0123456789ABCDEF";

        (0..4)
            .map(|_| {
                let index = rng.gen_range(0..charset.len());
                charset[index] as char
            })
            .collect()
    }

    fn print_result(mac: &str, cni_netns: &str, container_addr: &str, bridge_ip: &str) {
        let add_result = AddResult::new(
            mac.to_string(),
            cni_netns.to_string(),
            container_addr.to_string(),
            bridge_ip.to_string(),
        );
        let add_result_json = serde_json::to_string(&add_result).unwrap();

        println!("{}", add_result_json);
    }
}

#[derive(Serialize)]
pub struct AddResult {
    cni_version: String,
    interfaces: Vec<Interface>,
    ips: Vec<Ip>,
}

impl AddResult {
    pub fn new(mac: String, cni_netns: String, container_addr: String, bridge_ip: String) -> Self {
        Self {
            cni_version: "0.3.0".to_owned(),
            interfaces: vec![Interface::new(mac, cni_netns)],
            ips: vec![Ip::new(container_addr, bridge_ip)],
        }
    }
}

#[derive(Serialize)]
pub struct Interface {
    name: String,
    mac: String,
    sandbox: String,
}

impl Interface {
    pub fn new(mac: String, sandbox: String) -> Self {
        Self {
            name: "eth0".to_owned(),
            mac,
            sandbox,
        }
    }
}

#[derive(Serialize)]
pub struct Ip {
    version: String,
    address: String,
    gateway: String,
    interface: i32,
}

impl Ip {
    pub fn new(address: String, gateway: String) -> Self {
        Self {
            version: "4".to_owned(),
            address,
            gateway,
            interface: 0,
        }
    }
}
