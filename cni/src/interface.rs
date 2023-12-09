use serde::Serialize;

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
