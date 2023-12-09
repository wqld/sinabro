use serde::Serialize;

use crate::{interface::Interface, ip::Ip};

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
