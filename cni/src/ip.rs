use serde::Serialize;

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
