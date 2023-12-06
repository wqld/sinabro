use std::{borrow::BorrowMut, collections::BTreeSet, env, io, ops::DerefMut, sync::Mutex};

use ipnet::IpNet;
use once_cell::sync::{Lazy, OnceCell};
use sinabro::cni_config::CniConfig;
use tracing::{debug, info, warn};

static SUBNET: OnceCell<IpNet> = OnceCell::new();
static BRIDGE_IP: OnceCell<String> = OnceCell::new();
static IP_STORE: Lazy<Mutex<BTreeSet<String>>> = Lazy::new(|| match SUBNET.get() {
    Some(subnet) => subnet
        .hosts()
        .map(|ip| ip.to_string())
        .collect::<BTreeSet<String>>()
        .into(),
    None => BTreeSet::new().into(),
});

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Hello, world!");

    let command = env::var("CNI_COMMAND").unwrap_or_default();
    debug!("command: {command}");

    let stdin = io::read_to_string(io::stdin())?;
    debug!("stdin: {stdin}");

    let cni_config = CniConfig::from(stdin.as_str());

    SUBNET
        .set(cni_config.subnet.parse::<IpNet>().unwrap_or_default())
        .map_or_else(|_| warn!("setup subnet failed"), |_| {});

    BRIDGE_IP
        .set(IP_STORE.lock().unwrap().pop_first().unwrap_or_default())
        .map_or_else(|_| warn!("setup bridge ip failed"), |_| {});

    match command.as_str() {
        "ADD" => {
            let container_ip = IP_STORE.lock().unwrap().pop_first();
            debug!("container ip: {:?}", container_ip);
        }
        "DEL" => {}
        "VERSION" => {}
        _ => {}
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
