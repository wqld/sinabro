use std::{env, fs::File};

use anyhow::Result;
use async_trait::async_trait;
use nix::sched::{setns, CloneFlags};
use reqwest::Client;
use rsln::{
    netlink::Netlink,
    types::{addr::AddrFamily, link::LinkAttrs},
};
use sinabro_config::Config;
use tokio::task::spawn_blocking;
use tracing::{debug, info};

use super::CniCommand;

pub struct DeleteCommand;

#[async_trait]
impl CniCommand for DeleteCommand {
    async fn run(&self, _cni_config: &Config) -> Result<()> {
        let netns = env::var("CNI_NETNS")?;
        let netns_file = File::open(&netns)?;
        let cni_if_name = env::var("CNI_IFNAME")?;

        let client = Client::new();

        let container_ip = spawn_blocking(move || -> Result<Option<String>> {
            setns(netns_file, CloneFlags::CLONE_NEWNET)?;

            let mut netlink = Netlink::new();

            let link = match netlink.link_get(&LinkAttrs::new(&cni_if_name)) {
                Ok(link) => link,
                Err(_) => {
                    info!("(DELETE) link not found");
                    return Ok(None);
                }
            };

            let addr_list = match netlink.addr_list(&link, AddrFamily::V4) {
                Ok(addr_list) => addr_list,
                Err(_) => {
                    info!("(DELETE) addr not found");
                    return Ok(None);
                }
            };

            let container_ip = addr_list
                .first()
                .map(|addr| addr.ip.addr().to_string())
                .unwrap_or_default();

            Ok(Some(container_ip.to_owned()))
        })
        .await??;

        if let Some(ip) = container_ip {
            debug!("(DELETE) container ip: {}", ip);

            client
                .put(format!("http://localhost:3000/ipam/ip/{}", ip))
                .send()
                .await?;
        }

        Ok(())
    }
}
