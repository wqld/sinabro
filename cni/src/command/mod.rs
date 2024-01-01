use async_trait::async_trait;
use sinabro_cni::Config;

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
