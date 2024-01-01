use std::{env, fs, os::unix};

use async_trait::async_trait;
use ipnet::IpNet;
use rand::Rng;
use serde::Serialize;
use sinabro_cni::Config;
use tracing::{debug, error, info};

use super::CniCommand;

pub struct AddCommand;

#[async_trait]
impl CniCommand for AddCommand {
    async fn run(&self, cni_config: &Config) -> anyhow::Result<()> {
        let netns = env::var("CNI_NETNS")?;
        let container_id = env::var("CNI_CONTAINERID")?;
        let cni_if_name = env::var("CNI_IFNAME")?;
        let container_ip = Self::request_container_ip().await?;
        let subnet_mask_size = cni_config.subnet.split('/').last().unwrap();
        let container_addr = format!("{}/{}", container_ip, subnet_mask_size);
        debug!("container ip: {:?}", container_ip);

        let netns_path = "/var/run/netns";
        let symlink_netns_path = format!("{}/{}", netns_path, container_id);

        fs::create_dir_all(netns_path)?;
        unix::fs::symlink(&netns, symlink_netns_path)?;

        let unique_veth = Self::generate_veth_suffix();
        let veth_name = format!("veth{}", unique_veth);
        let peer_name = format!("peer{}", unique_veth);

        Self::run_command(
            "ip",
            &[
                "link", "add", &peer_name, "type", "veth", "peer", "name", &veth_name,
            ],
        )?;

        Self::run_command("ip", &["link", "set", &veth_name, "up"])?;
        Self::run_command("ip", &["link", "set", &veth_name, "master", "cni0"])?;

        Self::run_command("ip", &["link", "set", &peer_name, "netns", &container_id])?;

        Self::run_command(
            "ip",
            &[
                "netns",
                "exec",
                &container_id,
                "ip",
                "link",
                "set",
                &peer_name,
                "name",
                &cni_if_name,
            ],
        )?;
        Self::run_command(
            "ip",
            &[
                "netns",
                "exec",
                &container_id,
                "ip",
                "link",
                "set",
                &cni_if_name,
                "up",
            ],
        )?;

        Self::run_command(
            "ip",
            &[
                "netns",
                "exec",
                &container_id,
                "ip",
                "addr",
                "add",
                &container_addr,
                "dev",
                &cni_if_name,
            ],
        )?;

        let subnet = cni_config.subnet.parse::<IpNet>()?;
        let bridge_ip = subnet
            .hosts()
            .next()
            .map(|ip| ip.to_string())
            .ok_or_else(|| anyhow::anyhow!("failed to get bridge ip"))?;

        Self::run_command(
            "ip",
            &[
                "netns",
                "exec",
                &container_id,
                "ip",
                "route",
                "add",
                "default",
                "via",
                &bridge_ip,
                "dev",
                &cni_if_name,
            ],
        )?;

        let mac_addr = Self::get_mac_addr(&container_id)?;

        Self::print_result(&mac_addr, &netns, &container_addr, &bridge_ip);

        Ok(())
    }
}

impl AddCommand {
    async fn request_container_ip() -> anyhow::Result<String> {
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

    fn get_mac_addr(container_id: &str) -> anyhow::Result<String> {
        let output = std::process::Command::new("ip")
            .args(["netns", "exec", container_id, "ip", "link", "show", "eth0"])
            .output()?;

        let output_str = std::str::from_utf8(&output.stdout).unwrap();
        let mac = output_str
            .lines()
            .find(|line| line.contains("ether"))
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("");

        debug!("{}", mac);

        Ok(mac.to_string())
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
