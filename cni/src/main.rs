mod command;

use std::{env, io};

use sinabro_config::Config;
use tracing::{debug, error, Level};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _guard =
        sinabro_config::setup_tracing_to_file("/var/log", "sinabro-cni.log", Level::DEBUG)?;

    let command = env::var("CNI_COMMAND")?;
    debug!("command: {:?}", command);

    let stdin = io::read_to_string(io::stdin())?;
    debug!("stdin: {stdin}");

    let cni_config = Config::from(stdin.as_str());
    let cni_command = command::cni_command_from(&command)?;
    cni_command.run(&cni_config).await.map_err(|e| {
        error!("error: {:?}", e);
        e
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use ipnet::IpNet;

    #[test]
    fn cni_config_from_json() {
        let subnet = "10.244.0.0/24";
        let pod_cidr = subnet.parse::<IpNet>().unwrap();
        let count = pod_cidr.hosts().skip(1).count();
        assert_eq!(count, 253);
    }
}
