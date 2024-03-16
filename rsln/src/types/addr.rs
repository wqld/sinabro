use std::net::IpAddr;

use anyhow::Result;
use derive_builder::Builder;
use ipnet::IpNet;

use super::{
    message::{AddressMessage, Attribute, RouteAttrs},
    vec_to_addr,
};

pub enum AddrCmd {
    Add,
    Change,
    Replace,
    Delete,
}

pub enum AddrFamily {
    All = 0,
    V4 = 2,
    V6 = 10,
}

impl From<AddrFamily> for i32 {
    fn from(val: AddrFamily) -> Self {
        val as i32
    }
}

impl From<u16> for AddrFamily {
    fn from(val: u16) -> Self {
        match val {
            2 => Self::V4,
            10 => Self::V6,
            _ => Self::All,
        }
    }
}

#[derive(Default, Builder, Debug)]
#[builder(default)]
pub struct Address {
    pub index: i32,
    pub ip: IpNet,
    pub label: String,
    pub flags: u8,
    pub scope: u8,
    pub broadcast: Option<IpAddr>,
    pub peer: Option<IpNet>,
    pub preferred_lifetime: i32,
    pub valid_lifetime: i32,
}

impl From<&[u8]> for Address {
    fn from(buf: &[u8]) -> Self {
        let addr_msg: AddressMessage = bincode::deserialize(buf).unwrap();
        let attrs = RouteAttrs::from(&buf[addr_msg.len()..]);

        let mut addr = Self {
            index: addr_msg.index,
            scope: addr_msg.scope,
            ..Default::default()
        };

        for attr in attrs {
            match attr.header.rta_type {
                libc::IFA_ADDRESS => {
                    addr.update_address(&attr.payload, addr_msg.prefix_len)
                        .unwrap();
                }
                libc::IFA_LOCAL => {}
                _ => {}
            }
        }

        addr
    }
}

impl Address {
    pub fn update_address(&mut self, payload: &[u8], prefix_len: u8) -> Result<()> {
        let ip = vec_to_addr(payload)?;
        self.ip = IpNet::new(ip, prefix_len)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::types::message::{Payload, RouteAttr, RouteAttrHeader};

    use super::*;

    #[test]
    fn test_address_builder() {
        let address = AddressBuilder::default().build().unwrap();
        assert_eq!(address.index, 0);
    }

    #[test]
    fn test_update_address_ipv4() {
        let mut address = Address::default();
        let payload = Payload::from(&[192, 168, 1, 1][..]);
        let prefix_len = 24;

        address.update_address(&payload, prefix_len).unwrap();

        assert_eq!(address.ip, IpNet::V4("192.168.1.1/24".parse().unwrap()));
    }

    #[test]
    fn test_update_address_ipv6() {
        let mut address = Address::default();
        let payload = vec![
            0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0x02, 0x60, 0x97, 0xff, 0xfe, 0x07, 0x69, 0xea,
        ];
        let prefix_len = 64;

        address.update_address(&payload, prefix_len).unwrap();

        assert_eq!(
            address.ip,
            IpNet::V6("fe80::260:97ff:fe07:69ea/64".parse().unwrap())
        );
    }

    #[test]
    fn test_from_bytes() {
        let addr_msg = AddressMessage {
            index: 1,
            scope: 2,
            prefix_len: 24,
            ..Default::default()
        };
        let mut rt_attrs = RouteAttrs::default();
        rt_attrs.push(RouteAttr {
            header: RouteAttrHeader {
                rta_type: libc::IFA_ADDRESS,
                rta_len: 8,
            },
            payload: Payload::from(&[192, 168, 1, 1][..]),
            attributes: None,
        });

        let mut buf = AddressMessage::serialize(&addr_msg).unwrap();
        buf.extend_from_slice(RouteAttrs::serialize(&rt_attrs).unwrap().as_slice());

        let address = Address::from(&buf[..]);

        assert_eq!(address.index, addr_msg.index);
        assert_eq!(address.scope, addr_msg.scope);
    }
}
