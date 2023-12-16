pub mod add_result;
mod interface;
mod ip;

use std::{env, fs, io, os::unix};

use ipnet::IpNet;
use rand::Rng;
use sinabro::cni_config::CniConfig;
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let file_appender = tracing_appender::rolling::hourly("/var/log", "sinabro-cni.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let command = env::var("CNI_COMMAND")?;
    debug!("command: {command}");

    let stdin = io::read_to_string(io::stdin())?;
    debug!("stdin: {stdin}");

    let cni_config = CniConfig::from(stdin.as_str());

    match command.as_str() {
        "ADD" => add(cni_config.subnet).await,
        "DEL" => delete().await,
        "VERSION" => todo!(),
        _ => todo!(),
    }
}

async fn delete() -> anyhow::Result<()> {
    let container_id = env::var("CNI_CONTAINERID")?;
    let output = std::process::Command::new("ip")
        .args(["netns", "exec", &container_id, "ip", "addr", "show", "eth0"])
        .output()?;

    let output_str = std::str::from_utf8(&output.stdout).unwrap();
    let container_ip = output_str
        .lines()
        .find(|line| line.contains("inet"))
        .and_then(|line| line.split_whitespace().nth(1))
        .map(|ip| ip.split('/').next().unwrap())
        .unwrap_or("");

    debug!("(DELETE) container ip: {}", container_ip);

    reqwest::Client::new()
        .put(format!("http://localhost:3000/ipam/ip/{}", container_ip))
        .send()
        .await?;

    Ok(())
}

async fn add(subnet: &str) -> anyhow::Result<()> {
    let netns = env::var("CNI_NETNS")?;
    let container_id = env::var("CNI_CONTAINERID")?;
    let cni_if_name = env::var("CNI_IFNAME")?;
    let container_ip = request_container_ip().await?;
    let subnet_mask_size = subnet.split('/').last().unwrap();
    let container_addr = format!("{}/{}", container_ip, subnet_mask_size);
    debug!("container ip: {:?}", container_ip);

    let netns_path = "/var/run/netns";
    let symlink_netns_path = format!("{}/{}", netns_path, container_id);

    fs::create_dir_all(netns_path)?;
    unix::fs::symlink(&netns, symlink_netns_path)?;

    let unique_veth = generate_unique_veth();
    let veth_name = format!("veth{}", unique_veth);
    let peer_name = format!("peer{}", unique_veth);

    run_command(
        "ip",
        &[
            "link", "add", &peer_name, "type", "veth", "peer", "name", &veth_name,
        ],
    )?;

    run_command("ip", &["link", "set", &veth_name, "up"])?;
    run_command("ip", &["link", "set", &veth_name, "master", "cni0"])?;

    run_command("ip", &["link", "set", &peer_name, "netns", &container_id])?;

    run_command(
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
    run_command(
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

    run_command(
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

    let subnet = subnet.parse::<IpNet>()?;
    let bridge_ip = subnet
        .hosts()
        .next()
        .map(|ip| ip.to_string())
        .ok_or_else(|| anyhow::anyhow!("failed to get bridge ip"))?;

    run_command(
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

    let mac_addr = get_mac_addr(&container_id)?;

    print_result(&mac_addr, &netns, &container_addr, &bridge_ip);

    Ok(())
}

async fn request_container_ip() -> anyhow::Result<String> {
    let res = reqwest::get("http://localhost:3000/ipam/ip").await?;
    Ok(res.text().await?)
}

fn generate_unique_veth() -> String {
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
    let add_result = add_result::AddResult::new(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cni_config_from_json() {
        let subnet = "10.244.0.0/24";
        let pod_cidr = subnet.parse::<IpNet>().unwrap();
        let count = pod_cidr.hosts().skip(1).count();
        assert_eq!(count, 253);
    }
}
