use anyhow::Result;
use async_trait::async_trait;
use rand::Rng;
use sinabro_config::Config;

use self::{add::AddCommand, delete::DeleteCommand};

mod add;
mod delete;

#[async_trait]
pub trait CniCommand {
    async fn run(&self, cni_config: &Config) -> anyhow::Result<()>;
}

pub fn cni_command_from(command: &str) -> anyhow::Result<Box<dyn CniCommand>> {
    match command {
        "ADD" => Ok(Box::new(AddCommand)),
        "DEL" => Ok(Box::new(DeleteCommand)),
        _ => anyhow::bail!("unknown command: {}", command),
    }
}

pub fn generate_mac_addr() -> Result<Vec<u8>> {
    let mut rng = rand::thread_rng();
    let mut buf = [0u8; 6];
    rng.fill(&mut buf[..]);

    buf[0] = (buf[0] | 0x02) & 0xfe;

    Ok(buf.to_vec())
}
