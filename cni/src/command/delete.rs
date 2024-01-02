use std::env;

use async_trait::async_trait;
use sinabro_config::Config;
use tracing::debug;

use super::CniCommand;

pub struct DeleteCommand;

#[async_trait]
impl CniCommand for DeleteCommand {
    async fn run(&self, _cni_config: &Config) -> anyhow::Result<()> {
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
}
