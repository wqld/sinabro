use std::{
    collections::HashMap,
    mem,
    ops::{Deref, DerefMut},
    vec,
};

use anyhow::Result;
use bincode::deserialize;
use serde::{Deserialize, Serialize};

use crate::{
    align_of,
    handle::zero_terminated,
    types::{
        IFLA_VXLAN_AGEING, IFLA_VXLAN_FLOWBASED, IFLA_VXLAN_GBP, IFLA_VXLAN_GROUP,
        IFLA_VXLAN_GROUP6, IFLA_VXLAN_ID, IFLA_VXLAN_L2MISS, IFLA_VXLAN_L3MISS,
        IFLA_VXLAN_LEARNING, IFLA_VXLAN_LIMIT, IFLA_VXLAN_LINK, IFLA_VXLAN_LOCAL,
        IFLA_VXLAN_LOCAL6, IFLA_VXLAN_PORT, IFLA_VXLAN_PORT_RANGE, IFLA_VXLAN_PROXY,
        IFLA_VXLAN_RSC, IFLA_VXLAN_TOS, IFLA_VXLAN_TTL, IFLA_VXLAN_UDP_CSUM,
        IFLA_VXLAN_UDP_ZERO_CSUM6_RX, IFLA_VXLAN_UDP_ZERO_CSUM6_TX,
    },
};

use super::{
    link::{Kind, LinkAttrs, Namespace, VxlanAttrs},
    GENL_CTRL_CMD_GETFAMILY, GENL_CTRL_VERSION,
};

const RTA_ALIGNTO: usize = 0x4;
const RT_ATTR_HDR_SIZE: usize = 0x4;

const VETH_INFO_PEER: u16 = 1;

pub trait Attribute {
    fn len(&self) -> usize;

    fn serialize(&self) -> Result<Vec<u8>>;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub struct RouteAttrMap<'a>(HashMap<u16, &'a [u8]>);

impl<'a> From<&'a RouteAttrs> for RouteAttrMap<'a> {
    fn from(attrs: &'a RouteAttrs) -> Self {
        let map = attrs
            .iter()
            .map(|attr| (attr.header.rta_type, attr.payload.as_slice()))
            .collect();
        Self(map)
    }
}

impl<'a> Deref for RouteAttrMap<'a> {
    type Target = HashMap<u16, &'a [u8]>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl RouteAttrMap<'_> {
    pub fn get_bool(&self, key: &u16) -> Option<bool> {
        self.get(key).map(|v| v[0] == 1)
    }

    pub fn get_u8(&self, key: &u16) -> Option<u8> {
        self.get(key).map(|v| v[0])
    }

    pub fn get_u16(&self, key: &u16) -> Option<u16> {
        self.get(key)
            .map(|v| u16::from_ne_bytes(v[..2].try_into().unwrap_or([0; 2])))
    }

    pub fn get_u16_tuple(&self, key: &u16) -> Option<(u16, u16)> {
        self.get(key).map(|v| {
            (
                u16::from_ne_bytes(v[..2].try_into().unwrap_or([0; 2])),
                u16::from_ne_bytes(v[2..].try_into().unwrap_or([0; 2])),
            )
        })
    }

    pub fn get_u32(&self, key: &u16) -> Option<u32> {
        self.get(key)
            .map(|v| u32::from_ne_bytes(v[..4].try_into().unwrap_or([0; 4])))
    }

    pub fn get_vec(&self, key: &u16) -> Option<Vec<u8>> {
        self.get(key).map(|v| v.to_vec())
    }
}

#[derive(Default)]
pub struct RouteAttrs(Vec<RouteAttr>);

impl From<&[u8]> for RouteAttrs {
    fn from(mut buf: &[u8]) -> Self {
        let mut attrs = Vec::new();

        while buf.len() >= RT_ATTR_HDR_SIZE {
            let attr = RouteAttr::from(buf);
            let len = align_of(attr.header.rta_len as usize, RTA_ALIGNTO);
            attrs.push(attr);

            buf = &buf[len..];
        }

        Self(attrs)
    }
}

impl IntoIterator for RouteAttrs {
    type Item = RouteAttr;
    type IntoIter = vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Deref for RouteAttrs {
    type Target = Vec<RouteAttr>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for RouteAttrs {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl RouteAttrs {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        self.0
            .iter()
            .map(|attr| attr.serialize())
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.concat())
    }
}

#[derive(Default)]
pub struct RouteAttr {
    pub header: RouteAttrHeader,
    pub payload: Payload,
    pub attributes: Option<Vec<Box<dyn Attribute>>>,
}

impl Attribute for RouteAttr {
    fn len(&self) -> usize {
        self.header.rta_len as usize
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(self.len());

        buf.extend_from_slice(&self.header.rta_len.to_ne_bytes());
        buf.extend_from_slice(&self.header.rta_type.to_ne_bytes());
        buf.extend_from_slice(&self.payload);

        let align_to = align_of(buf.len(), RTA_ALIGNTO);

        if buf.len() < align_to {
            buf.resize(align_to, 0);
        }

        if let Some(attrs) = &self.attributes {
            for attr in attrs {
                buf.extend_from_slice(&attr.serialize()?);
            }

            let len = buf.len();
            buf[..2].copy_from_slice(&(len as u16).to_ne_bytes());
        }

        Ok(buf)
    }
}

impl From<&[u8]> for RouteAttr {
    fn from(buf: &[u8]) -> Self {
        let header: RouteAttrHeader = deserialize(buf).expect("Failed to deserialize header");
        let payload = Payload::from(&buf[RT_ATTR_HDR_SIZE..header.rta_len as usize]);

        Self {
            header,
            payload,
            attributes: None,
        }
    }
}

pub const BR_HELLO_TIME: u16 = 0x2;
pub const BR_AGEING_TIME: u16 = 0x4;
pub const BR_VLAN_FILTERING: u16 = 0x7;
pub const BR_MCAST_SNOOPING: u16 = 0x17;

impl From<&Kind> for Option<RouteAttr> {
    fn from(kind: &Kind) -> Self {
        match kind {
            Kind::Bridge {
                attrs: _,
                hello_time: ht,
                ageing_time: at,
                vlan_filtering: vf,
                multicast_snooping: ms,
            } => RouteAttr::from_bridge(ht, at, vf, ms),
            Kind::Veth {
                attrs: base,
                peer_name,
                peer_hw_addr,
                peer_ns,
            } => RouteAttr::from_veth(base, peer_name, peer_hw_addr, peer_ns),
            Kind::Vxlan {
                attrs: _,
                vxlan_attrs,
            } => RouteAttr::from_vxlan(vxlan_attrs),
            _ => None,
        }
    }
}

impl RouteAttr {
    pub fn new(rta_type: u16, payload: &[u8]) -> Self {
        Self::with_attrs(rta_type, payload, None)
    }

    pub fn from_bridge(
        ht: &Option<u32>,
        at: &Option<u32>,
        vf: &Option<bool>,
        ms: &Option<bool>,
    ) -> Option<Self> {
        let sub_attrs = {
            let candidates = [
                ht.map(|v| RouteAttr::new(BR_HELLO_TIME, &v.to_ne_bytes())),
                at.map(|v| RouteAttr::new(BR_AGEING_TIME, &v.to_ne_bytes())),
                vf.map(|v| RouteAttr::new(BR_VLAN_FILTERING, &(v as u8).to_ne_bytes())),
                ms.map(|v| RouteAttr::new(BR_MCAST_SNOOPING, &(v as u8).to_ne_bytes())),
            ]
            .into_iter()
            .filter_map(|opt| opt.map(|ra| Box::new(ra) as Box<dyn Attribute>))
            .collect::<Vec<Box<dyn Attribute>>>();

            Some(candidates).filter(|vec| !vec.is_empty())
        };

        Some(Self::with_attrs(libc::IFLA_INFO_DATA, &[], sub_attrs))
    }

    pub fn from_veth(
        attrs: &LinkAttrs,
        peer_name: &str,
        peer_hw_addr: &Option<Vec<u8>>,
        peer_ns: &Option<Namespace>,
    ) -> Option<Self> {
        let mut sub_attrs = Vec::new();
        let mut peer_info = RouteAttr::new(VETH_INFO_PEER, &[]);

        peer_info.add_attribute(Box::new(LinkMessage::new(libc::AF_UNSPEC)));
        peer_info.add(libc::IFLA_IFNAME, &zero_terminated(peer_name));

        if attrs.mtu > 0 {
            peer_info.add(libc::IFLA_MTU, &attrs.mtu.to_ne_bytes());
        }

        if attrs.tx_queue_len >= 0 {
            peer_info.add(libc::IFLA_TXQLEN, &attrs.tx_queue_len.to_ne_bytes());
        }

        if attrs.num_tx_queues > 0 {
            peer_info.add(libc::IFLA_NUM_TX_QUEUES, &attrs.num_tx_queues.to_ne_bytes());
        }

        if attrs.num_rx_queues > 0 {
            peer_info.add(libc::IFLA_NUM_RX_QUEUES, &attrs.num_rx_queues.to_ne_bytes());
        }

        if let Some(hw_addr) = peer_hw_addr {
            peer_info.add(libc::IFLA_ADDRESS, hw_addr);
        }

        match peer_ns {
            Some(Namespace::Pid(pid)) => peer_info.add(libc::IFLA_NET_NS_PID, &pid.to_ne_bytes()),
            Some(Namespace::Fd(fd)) => peer_info.add(libc::IFLA_NET_NS_FD, &fd.to_ne_bytes()),
            _ => (),
        }

        sub_attrs.push(Box::new(peer_info) as Box<dyn Attribute>);

        Some(Self::with_attrs(libc::IFLA_INFO_DATA, &[], Some(sub_attrs)))
    }

    pub fn from_vxlan(vxlan_attrs: &VxlanAttrs) -> Option<Self> {
        let mut attrs = Vec::<Box<dyn Attribute>>::new();
        let mut id = vxlan_attrs.id;

        let mut add_attr = |cond: bool, rta_type: u16, payload: &[u8]| {
            if cond {
                attrs.push(Box::new(RouteAttr::new(rta_type, payload)));
            }
        };

        if vxlan_attrs.flow_based {
            id = 0;
        }

        add_attr(true, IFLA_VXLAN_ID, &id.to_ne_bytes());
        add_attr(
            vxlan_attrs.flow_based,
            IFLA_VXLAN_FLOWBASED,
            &[vxlan_attrs.flow_based as u8],
        );

        if let Some(vtep_index) = vxlan_attrs.vtep_index {
            add_attr(true, IFLA_VXLAN_LINK, &vtep_index.to_ne_bytes());
        }

        if let Some(group) = &vxlan_attrs.group {
            match group.len() {
                4 => add_attr(true, IFLA_VXLAN_GROUP, group.as_slice()),
                16 => add_attr(true, IFLA_VXLAN_GROUP6, group.as_slice()),
                _ => (),
            }
        }

        if let Some(src_addr) = &vxlan_attrs.src_addr {
            match src_addr.len() {
                4 => add_attr(true, IFLA_VXLAN_LOCAL, src_addr.as_slice()),
                16 => add_attr(true, IFLA_VXLAN_LOCAL6, src_addr.as_slice()),
                _ => (),
            }
        }

        add_attr(true, IFLA_VXLAN_TTL, &[vxlan_attrs.ttl]);
        add_attr(true, IFLA_VXLAN_TOS, &[vxlan_attrs.tos]);
        add_attr(true, IFLA_VXLAN_LEARNING, &[vxlan_attrs.learning as u8]);
        add_attr(true, IFLA_VXLAN_PROXY, &[vxlan_attrs.proxy as u8]);
        add_attr(true, IFLA_VXLAN_RSC, &[vxlan_attrs.rsc as u8]);
        add_attr(true, IFLA_VXLAN_L2MISS, &[vxlan_attrs.l2miss as u8]);
        add_attr(true, IFLA_VXLAN_L3MISS, &[vxlan_attrs.l3miss as u8]);
        add_attr(
            true,
            IFLA_VXLAN_UDP_ZERO_CSUM6_TX,
            &[vxlan_attrs.udp_zero_csum6_tx as u8],
        );
        add_attr(
            true,
            IFLA_VXLAN_UDP_ZERO_CSUM6_RX,
            &[vxlan_attrs.udp_zero_csum6_rx as u8],
        );

        add_attr(
            vxlan_attrs.udp_csum,
            IFLA_VXLAN_UDP_CSUM,
            &[vxlan_attrs.udp_csum as u8],
        );
        add_attr(vxlan_attrs.gbp, IFLA_VXLAN_GBP, &[]);

        let ageing = match vxlan_attrs.ageing {
            Some(ageing) if ageing > 0 => ageing.to_ne_bytes(),
            _ => [0; 4],
        };
        add_attr(true, IFLA_VXLAN_AGEING, &ageing);

        if let Some(limit) = vxlan_attrs.limit {
            add_attr(limit > 0, IFLA_VXLAN_LIMIT, &limit.to_ne_bytes());
        }

        if let Some(port) = vxlan_attrs.port {
            add_attr(port > 0, IFLA_VXLAN_PORT, &port.to_be_bytes());
        }

        if let Some((low, high)) = vxlan_attrs.port_range {
            if low > 0 || high > 0 {
                let mut buf = [0; 4];
                buf[..2].copy_from_slice(&low.to_ne_bytes());
                buf[2..].copy_from_slice(&high.to_ne_bytes());
                add_attr(true, IFLA_VXLAN_PORT_RANGE, &buf);
            }
        }

        Some(Self::with_attrs(libc::IFLA_INFO_DATA, &[], Some(attrs)))
    }

    fn with_attrs(rta_type: u16, payload: &[u8], attrs: Option<Vec<Box<dyn Attribute>>>) -> Self {
        Self {
            header: RouteAttrHeader {
                rta_len: (RT_ATTR_HDR_SIZE + payload.len()) as u16,
                rta_type,
            },
            payload: Payload::from(payload),
            attributes: attrs,
        }
    }

    pub fn add(&mut self, rta_type: u16, payload: &[u8]) {
        let attr = RouteAttr::new(rta_type, payload);
        self.add_attribute(Box::new(attr));
    }

    pub fn add_attribute(&mut self, attr: Box<dyn Attribute>) {
        self.header.rta_len += attr.len() as u16;

        match &mut self.attributes {
            None => self.attributes = Some(vec![attr]),
            Some(attrs) => attrs.push(attr),
        }
    }
}

#[repr(C)]
#[derive(Serialize, Deserialize, Default)]
pub struct RouteAttrHeader {
    pub rta_len: u16,
    pub rta_type: u16, // TODO: use enum
}

/// TODO: `Payload` should be changed to use `&'a mut [u8]` instead of `Vec<u8>`
#[derive(Default, Debug, PartialEq)]
pub struct Payload(Vec<u8>);

impl From<&[u8]> for Payload {
    fn from(buf: &[u8]) -> Self {
        Self(buf.to_vec())
    }
}

impl Deref for Payload {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Payload {
    pub fn to_string(&self) -> Result<String> {
        let mut buf = self.to_vec();
        buf.truncate(self.len() - 1);
        String::from_utf8(buf).map_err(|e| e.into())
    }

    pub fn to_u16(&self) -> Result<u16> {
        let mut buf = self.to_vec();
        buf.truncate(2);
        Ok(u16::from_ne_bytes(buf.try_into().unwrap()))
    }

    pub fn to_u32(&self) -> Result<u32> {
        let mut buf = self.to_vec();
        buf.truncate(4);
        Ok(u32::from_ne_bytes(buf.try_into().unwrap()))
    }

    pub fn to_i32(&self) -> Result<i32> {
        let mut buf = self.to_vec();
        buf.truncate(4);
        Ok(i32::from_ne_bytes(buf.try_into().unwrap()))
    }
}

#[repr(C)]
#[derive(Serialize, Deserialize, Default)]
pub struct LinkMessage {
    pub family: u8,
    pub _pad: u8,
    pub dev_type: u16,
    pub index: i32,
    pub flags: u32,
    pub change_mask: u32,
}

impl Attribute for LinkMessage {
    fn len(&self) -> usize {
        16
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(bincode::serialize(self)?)
    }
}

impl LinkMessage {
    pub fn new(family: i32) -> Self {
        Self {
            family: family as u8,
            ..Default::default()
        }
    }
}

#[repr(C)]
#[derive(Serialize, Deserialize, Default)]
pub struct AddressMessage {
    pub family: u8,
    pub prefix_len: u8,
    pub flags: u8,
    pub scope: u8,
    pub index: i32,
}

impl Attribute for AddressMessage {
    fn len(&self) -> usize {
        8
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(bincode::serialize(self)?)
    }
}

impl AddressMessage {
    pub fn new(family: i32) -> Self {
        Self {
            family: family as u8,
            ..Default::default()
        }
    }
}

#[repr(C)]
#[derive(Serialize, Deserialize, Default)]
pub struct RouteMessage {
    pub family: u8,
    pub dst_len: u8,
    pub src_len: u8,
    pub tos: u8,
    pub table: u8,
    pub protocol: u8,
    pub scope: u8,
    pub route_type: u8,
    pub flags: u32,
}

impl Attribute for RouteMessage {
    fn len(&self) -> usize {
        12
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(bincode::serialize(self)?)
    }
}

impl RouteMessage {
    pub fn new() -> Self {
        Self {
            table: libc::RT_TABLE_MAIN,
            protocol: libc::RTPROT_BOOT,
            scope: libc::RT_SCOPE_UNIVERSE,
            route_type: libc::RTN_UNICAST,
            ..Default::default()
        }
    }

    pub fn new_delete_msg() -> Self {
        Self {
            table: libc::RT_TABLE_MAIN,
            scope: libc::RT_SCOPE_NOWHERE,
            ..Default::default()
        }
    }
}

#[repr(C)]
#[derive(Serialize, Deserialize, Default)]
pub struct NeighborMessage {
    pub family: u8,
    pub _pad: [u8; 3],
    pub index: u32,
    pub state: u16,
    pub flags: u8,
    pub neigh_type: u8,
}

impl Attribute for NeighborMessage {
    fn len(&self) -> usize {
        12
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(bincode::serialize(self)?)
    }
}

impl NeighborMessage {
    pub fn new(family: u8, index: u32, state: u16, flags: u8, neigh_type: u8) -> Self {
        Self {
            family,
            _pad: [0; 3],
            index,
            state,
            flags,
            neigh_type,
        }
    }
}

#[repr(C)]
#[derive(Serialize, Deserialize, Default)]
pub struct GenlMessage {
    pub command: u8,
    pub version: u8,
}

impl Attribute for GenlMessage {
    fn len(&self) -> usize {
        4
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(bincode::serialize(self)?)
    }
}

impl GenlMessage {
    pub fn get_family_message() -> Self {
        Self {
            command: GENL_CTRL_CMD_GETFAMILY,
            version: GENL_CTRL_VERSION,
        }
    }
}

pub struct Buffer<'a>(&'a mut [u8]);

impl<'a> From<&'a mut [u8]> for Buffer<'a> {
    fn from(buf: &'a mut [u8]) -> Self {
        Self(buf)
    }
}

impl<'a> Deref for Buffer<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a> Buffer<'a> {
    pub fn take<'s>(&'s mut self, len: usize) -> Option<&'a mut [u8]> {
        if len > self.len() {
            return None;
        }

        let buf = mem::take(&mut self.0);
        let (taken, rest) = buf.split_at_mut(len);

        self.0 = rest;

        Some(taken)
    }
}

#[cfg(test)]
mod tests {
    use crate::types::message::LinkMessage;
    use crate::types::message::RouteAttrHeader;

    use super::*;

    struct TestAttribute {
        len: usize,
    }

    impl Attribute for TestAttribute {
        fn len(&self) -> usize {
            self.len
        }

        fn serialize(&self) -> Result<Vec<u8>> {
            Ok(vec![0; self.len])
        }
    }

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
    fn test_link_header_deserialize() {
        let msg: LinkMessage = bincode::deserialize(&NETLINK_MSG).unwrap();

        assert_eq!(msg.family, 0);
        assert_eq!(msg.dev_type, 772);
        assert_eq!(msg.index, 1);
        assert_eq!(
            msg.flags,
            libc::IFF_UP as u32 | libc::IFF_LOOPBACK as u32 | libc::IFF_RUNNING as u32
        );
        assert_eq!(msg.change_mask, 0);
    }

    #[test]
    fn test_route_attr_serialize() {
        let header = RouteAttrHeader {
            rta_len: 20,
            rta_type: 1,
        };
        let payload = Payload::from(&[1, 2, 3][..]);
        let attributes: Option<Vec<Box<dyn Attribute>>> = None;

        let route_attr = RouteAttr {
            header,
            payload,
            attributes,
        };

        let serialized = Attribute::serialize(&route_attr).unwrap();

        assert_eq!(serialized, vec![20, 0, 1, 0, 1, 2, 3, 0]);
    }

    #[test]
    fn test_link_message_serialize() {
        let link_message = LinkMessage {
            family: 1,
            _pad: 0,
            dev_type: 2,
            index: 3,
            flags: 4,
            change_mask: 5,
        };

        let serialized = Attribute::serialize(&link_message).unwrap();

        // Assert the serialized bytes are correct
        assert_eq!(
            serialized,
            vec![1, 0, 2, 0, 3, 0, 0, 0, 4, 0, 0, 0, 5, 0, 0, 0]
        );
    }

    #[test]
    fn test_route_attrs_from() {
        let route_attrs = RouteAttrs::from(&NETLINK_MSG[16..]);
        assert_eq!(route_attrs.len(), 10);
    }

    #[test]
    fn test_route_attr_new() {
        let payload = Payload::from(&[0; 10][..]);
        let attr = RouteAttr::new(1, &payload);

        assert_eq!(
            attr.header.rta_len,
            (RT_ATTR_HDR_SIZE + payload.len()) as u16
        );
        assert_eq!(attr.header.rta_type, 1);
        assert_eq!(attr.payload, payload);
        assert!(attr.attributes.is_none());
    }

    #[test]
    fn test_add_attribute() {
        let mut attr = RouteAttr::new(1, &[0; 10][..]);
        let test_attr = Box::new(TestAttribute { len: 5 });

        attr.add_attribute(test_attr);

        assert_eq!(attr.header.rta_len, (RT_ATTR_HDR_SIZE + 10 + 5) as u16);
        assert!(attr.attributes.is_some());

        let attributes = attr.attributes.unwrap();
        assert_eq!(attributes.len(), 1);
        assert_eq!(attributes[0].len(), 5);
    }
}
