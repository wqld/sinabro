use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use anyhow::Result;
use derive_builder::Builder;
use ipnet::IpNet;

use crate::RTA_VIA;

use super::{
    addr::AddrFamily,
    message::{Attribute, RouteAttrs, RouteMessage},
    vec_to_addr,
};

pub enum RtCmd {
    Add,
    Append,
    Replace,
    Delete,
}

#[derive(Default, Builder)]
#[builder(default)]
pub struct Routing {
    pub oif_index: i32,
    pub iif_index: i32,
    pub family: u8,
    pub dst: Option<IpNet>,
    pub src: Option<IpAddr>,
    pub gw: Option<IpAddr>,
    pub tos: u8,
    pub table: u8,
    pub protocol: u8,
    pub scope: u8,
    pub rtm_type: u8,
    pub via: Option<Via>,
    pub mtu: Option<u32>,
    pub flags: u32,
}

impl From<&[u8]> for Routing {
    fn from(buf: &[u8]) -> Self {
        let rt_msg: RouteMessage = bincode::deserialize(buf).unwrap();
        let rt_attrs = RouteAttrs::from(&buf[rt_msg.len()..]);

        let mut routing = Self {
            family: rt_msg.family,
            tos: rt_msg.tos,
            table: rt_msg.table,
            protocol: rt_msg.protocol,
            scope: rt_msg.scope,
            rtm_type: rt_msg.route_type,
            ..Default::default()
        };

        for attr in rt_attrs {
            match attr.header.rta_type {
                libc::RTA_GATEWAY => {
                    routing.gw = Some(vec_to_addr(&attr.payload).unwrap());
                }
                libc::RTA_PREFSRC => {
                    routing.src = Some(vec_to_addr(&attr.payload).unwrap());
                }
                libc::RTA_DST => {
                    routing.dst = Some(
                        IpNet::new(vec_to_addr(&attr.payload).unwrap(), rt_msg.dst_len).unwrap(),
                    );
                }
                libc::RTA_OIF => {
                    routing.oif_index = i32::from_ne_bytes(attr.payload[..4].try_into().unwrap());
                }
                libc::RTA_IIF => {
                    routing.iif_index = i32::from_ne_bytes(attr.payload[..4].try_into().unwrap());
                }
                libc::RTA_TABLE => {
                    routing.table = u8::from_ne_bytes(attr.payload[..1].try_into().unwrap());
                }
                RTA_VIA => {
                    let family = u16::from_ne_bytes(attr.payload[..2].try_into().unwrap());
                    let addr = vec_to_addr(&attr.payload[2..]).unwrap();
                    routing.via = Some(Via { family, addr });
                }
                _ => {}
            }
        }

        routing
    }
}

#[derive(Clone)]
pub struct Via {
    pub family: u16,
    pub addr: IpAddr,
}

impl Via {
    pub fn new(addr: &str) -> Result<Self> {
        let (family, addr) = match addr.parse::<Ipv4Addr>() {
            Ok(ip) => (AddrFamily::V4 as u16, IpAddr::V4(ip)),
            Err(_) => {
                let ip = addr.parse::<Ipv6Addr>()?;
                (AddrFamily::V6 as u16, IpAddr::V6(ip))
            }
        };

        Ok(Self { family, addr })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.family.to_ne_bytes());
        match self.addr {
            IpAddr::V4(ip) => {
                buf.extend_from_slice(&ip.octets());
            }
            IpAddr::V6(ip) => {
                buf.extend_from_slice(&ip.octets());
            }
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use crate::types::message::{Payload, RouteAttr, RouteAttrHeader};

    use super::*;

    #[test]
    fn test_from_bytes() {
        let rt_msg = RouteMessage {
            family: 2,
            tos: 0,
            table: 0,
            protocol: 0,
            scope: 0,
            route_type: 0,
            dst_len: 32,
            ..Default::default()
        };
        let mut rt_attrs = RouteAttrs::default();
        rt_attrs.push(RouteAttr {
            header: RouteAttrHeader {
                rta_type: libc::RTA_DST,
                rta_len: 8,
            },
            payload: Payload::from(&[192, 168, 1, 1][..]),
            attributes: None,
        });

        let mut buf = RouteMessage::serialize(&rt_msg).unwrap();
        buf.extend_from_slice(RouteAttrs::serialize(&rt_attrs).unwrap().as_slice());

        let routing = Routing::from(&buf[..]);

        assert_eq!(routing.family, rt_msg.family);
        assert_eq!(routing.tos, rt_msg.tos);
        assert_eq!(routing.table, rt_msg.table);
        assert_eq!(routing.protocol, rt_msg.protocol);
        assert_eq!(routing.scope, rt_msg.scope);
        assert_eq!(routing.rtm_type, rt_msg.route_type);
        assert_eq!(
            routing.dst,
            Some(IpNet::V4("192.168.1.1/32".parse().unwrap()))
        );
    }
}
