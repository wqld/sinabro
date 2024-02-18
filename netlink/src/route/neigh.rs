use std::net::IpAddr;

use anyhow::Result;
use derive_builder::Builder;

use crate::route::message::{Attribute, NeighborMessage, RouteAttrs};

use super::vec_to_addr;

#[derive(Default, Builder)]
#[builder(default, build_fn(validate = "Self::validate"))]
pub struct Neighbor {
    pub link_index: u32,
    pub family: Option<u8>,
    pub state: u16,
    pub ip_addr: Option<IpAddr>,
    pub mac_addr: Option<Vec<u8>>,
    pub neigh_type: u8,
    pub flags: u8,
}

impl From<&[u8]> for Neighbor {
    fn from(buf: &[u8]) -> Self {
        let neigh_msg: NeighborMessage = bincode::deserialize(buf).unwrap();
        let rt_attrs = RouteAttrs::from(&buf[neigh_msg.len()..]);

        let mut neighbor = Self {
            link_index: neigh_msg.index,
            family: Some(neigh_msg.family),
            state: neigh_msg.state,
            neigh_type: neigh_msg.neigh_type,
            flags: neigh_msg.flags,
            ..Default::default()
        };

        for attr in rt_attrs {
            match attr.header.rta_type {
                libc::NDA_DST => {
                    neighbor.ip_addr = Some(vec_to_addr(&attr.payload).unwrap());
                }
                libc::NDA_LLADDR => {
                    neighbor.mac_addr = Some(attr.payload.to_vec());
                }
                _ => {}
            }
        }

        neighbor
    }
}

impl NeighborBuilder {
    fn validate(&self) -> Result<(), String> {
        if self.ip_addr.is_none() {
            return Err("IP address is required".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::route::message::{Payload, RouteAttr, RouteAttrHeader};

    use super::*;

    #[test]
    fn test_neighbor_builder_default_returns_error() {
        let neighbor = NeighborBuilder::default().build();
        assert!(neighbor.is_err());
    }

    #[test]
    fn test_neighbor_builder_arp() {
        let _ = NeighborBuilder::default()
            .link_index(5)
            .state(128)
            .ip_addr(Some(IpAddr::V4("10.244.1.0".parse().unwrap())))
            .mac_addr(Some(vec![0x02, 0x12, 0x34, 0x56, 0x78, 0x9A]))
            .neigh_type(1)
            .build()
            .unwrap();
    }

    #[test]
    fn test_neighbor_build_fdb() {
        let _ = NeighborBuilder::default()
            .link_index(5)
            .state(128)
            .ip_addr(Some(IpAddr::V4("10.244.1.0".parse().unwrap())))
            .mac_addr(Some(vec![0x02, 0x12, 0x34, 0x56, 0x78, 0x9A]))
            .family(Some(7))
            .flags(2)
            .build()
            .unwrap();
    }

    #[test]
    fn test_from_bytes() {
        let neigh_msg = NeighborMessage {
            family: libc::AF_INET as u8,
            index: 5,
            state: 128,
            neigh_type: 1,
            ..Default::default()
        };
        let mut rt_attrs = RouteAttrs::default();
        rt_attrs.push(RouteAttr {
            header: RouteAttrHeader {
                rta_type: libc::NDA_DST,
                rta_len: 8,
            },
            payload: Payload::from(&[10, 244, 1, 0][..]),
            attributes: None,
        });
        rt_attrs.push(RouteAttr {
            header: RouteAttrHeader {
                rta_type: libc::NDA_LLADDR,
                rta_len: 10,
            },
            payload: Payload::from(&[0x02, 0x12, 0x34, 0x56, 0x78, 0x9A][..]),
            attributes: None,
        });

        let mut buf = NeighborMessage::serialize(&neigh_msg).unwrap();
        buf.extend_from_slice(RouteAttrs::serialize(&rt_attrs).unwrap().as_slice());

        let neighbor = Neighbor::from(&buf[..]);

        assert_eq!(neighbor.link_index, neigh_msg.index);
    }
}
