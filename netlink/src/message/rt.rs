use std::ops::Deref;

use anyhow::Result;
use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use crate::{
    consts::{self, AttributeKind},
    utils::align_of,
};

pub trait NetlinkPayload {
    fn size(&self) -> usize;
    fn serialize(&self) -> Result<Vec<u8>>;
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Builder, Serialize, Deserialize)]
pub struct LinkHeader {
    pub family: u8,
    #[builder(default)]
    pub _pad: u8,
    #[builder(default)]
    pub kind: u16,
    #[builder(default)]
    pub index: i32,
    #[builder(default)]
    pub flags: u32,
    #[builder(default)]
    pub change: u32,
}

impl NetlinkPayload for LinkHeader {
    fn size(&self) -> usize {
        consts::IF_INFO_MSG_SIZE
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| e.into())
    }
}

#[repr(C)]
#[derive(Debug, Default, Builder, Serialize, Deserialize)]
pub struct AddressHeader {
    pub family: u8,
    pub prefix_len: u8,
    pub flags: u8,
    pub scope: u8,
    pub index: i32,
}

impl NetlinkPayload for AddressHeader {
    fn size(&self) -> usize {
        consts::ADDR_MSG_SIZE
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| e.into())
    }
}

#[repr(C)]
#[derive(Debug, Default, Builder, Serialize, Deserialize)]
pub struct RouteHeader {
    pub family: u8,
    pub dst_len: u8,
    pub src_len: u8,
    pub tos: u8,
    pub table: u8,
    pub protocol: u8,
    pub scope: u8,
    pub kind: u8,
    pub flags: u32,
}

impl NetlinkPayload for RouteHeader {
    fn size(&self) -> usize {
        consts::ROUTE_MSG_SIZE
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| e.into())
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct Attribute {
    pub len: u16,
    pub kind: u16,
    pub payload: Vec<u8>,
}

impl Attribute {
    pub fn new(kind: AttributeKind, payload: &[u8]) -> Self {
        Self {
            len: (consts::RT_ATTR_SIZE + payload.len()) as u16,
            kind: kind as u16,
            payload: payload.to_vec(),
        }
    }

    pub fn append_payload<T: NetlinkPayload>(&mut self, attr: &T) -> Result<()> {
        self.payload.extend_from_slice(&attr.serialize()?);
        self.len += attr.size() as u16;
        Ok(())
    }
}

impl NetlinkPayload for Attribute {
    fn size(&self) -> usize {
        self.len as usize
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.len.to_ne_bytes());
        buf.extend_from_slice(&self.kind.to_ne_bytes());
        buf.extend_from_slice(&self.payload);

        let align_to = align_of(buf.len(), consts::RTA_ALIGNTO);
        if buf.len() < align_to {
            buf.resize(align_to, 0);
        }

        let len = buf.len();
        buf[..2].copy_from_slice(&(len as u16).to_ne_bytes());

        Ok(buf)
    }
}

pub struct Attributes(Vec<Attribute>);

impl Attributes {
    pub fn from(mut buf: &[u8]) -> Result<Self> {
        let mut attrs = Vec::new();

        while buf.len() >= consts::RT_ATTR_SIZE {
            let len = u16::from_ne_bytes(buf[..2].try_into()?);
            let kind = u16::from_ne_bytes(buf[2..4].try_into()?);
            let align_to = align_of(len as usize, consts::NLMSG_ALIGN_TO);
            let payload = buf[consts::RT_ATTR_SIZE..align_to].to_vec();

            attrs.push(Attribute { len, kind, payload });

            buf = &buf[align_to..];
        }

        Ok(Self(attrs))
    }
}

impl Deref for Attributes {
    type Target = Vec<Attribute>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rustfmt::skip]
    static NETLINK_MSG: [u8; 96] = [
        0x00, // interface family
        0x00, // reserved
        0x04, 0x03, // link layer type 772 = loopback
        0x01, 0x00, 0x00, 0x00, // interface index = 1
        0x49, 0x00, 0x00, 0x00, // device flags: UP, LOOPBACK, RUNNING, LOWERUP
        0x00, 0x00, 0x00, 0x00, // reserved 2 (aka device change flag)

        // nlas
        0x07, 0x00, 0x03, 0x00, 0x6c, 0x6f, 0x00, // device name L=7,T=3,V=lo
        0x00, // padding
        0x08, 0x00, 0x0d, 0x00, 0xe8, 0x03, 0x00, 0x00, // TxQueue length L=8,T=13,V=1000
        0x05, 0x00, 0x10, 0x00, 0x00, // OperState L=5,T=16,V=0 (unknown)
        0x00, 0x00, 0x00, // padding
        0x05, 0x00, 0x11, 0x00, 0x00, // Link mode L=5,T=17,V=0
        0x00, 0x00, 0x00, // padding
        0x08, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00, // MTU L=8,T=4,V=65536
        0x08, 0x00, 0x1b, 0x00, 0x00, 0x00, 0x00, 0x00, // Group L=8,T=27,V=9
        0x08, 0x00, 0x1e, 0x00, 0x00, 0x00, 0x00, 0x00, // Promiscuity L=8,T=30,V=0
        0x08, 0x00, 0x1f, 0x00, 0x01, 0x00, 0x00, 0x00, // Number of Tx Queues L=8,T=31,V=1
        0x08, 0x00, 0x28, 0x00, 0xff, 0xff, 0x00, 0x00, // Maximum GSO segment count L=8,T=40,V=65536
        0x08, 0x00, 0x29, 0x00, 0x00, 0x00, 0x01, 0x00, // Maximum GSO size L=8,T=41,V=65536
    ];

    #[test]
    fn test_link_header() {
        let msg = bincode::deserialize::<LinkHeader>(&NETLINK_MSG).unwrap();

        assert_eq!(772, msg.kind);
        assert_eq!(1, msg.index);
        assert_eq!(73, msg.flags);

        let attrs = Attributes::from(&NETLINK_MSG[msg.size()..]).unwrap();
        attrs.0.iter().for_each(|a| println!("{:?}", a));

        assert_eq!(10, attrs.len());
        assert_eq!(
            "lo",
            String::from_utf8_lossy(&attrs[0].payload).trim_matches(char::from(0))
        );
        assert_eq!(
            1000,
            u32::from_ne_bytes(attrs[1].payload[..].try_into().unwrap())
        );
    }
}
