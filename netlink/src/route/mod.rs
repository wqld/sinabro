use std::net::IpAddr;

use anyhow::Result;
use thiserror::Error;

pub mod addr;
pub mod link;
pub mod message;
pub mod neigh;
pub mod routing;

pub const IFLA_VXLAN_UNSPEC: u16 = 0;
pub const IFLA_VXLAN_ID: u16 = 1;
pub const IFLA_VXLAN_GROUP: u16 = 2;
pub const IFLA_VXLAN_LINK: u16 = 3;
pub const IFLA_VXLAN_LOCAL: u16 = 4;
pub const IFLA_VXLAN_TTL: u16 = 5;
pub const IFLA_VXLAN_TOS: u16 = 6;
pub const IFLA_VXLAN_LEARNING: u16 = 7;
pub const IFLA_VXLAN_AGEING: u16 = 8;
pub const IFLA_VXLAN_LIMIT: u16 = 9;
pub const IFLA_VXLAN_PORT_RANGE: u16 = 10;
pub const IFLA_VXLAN_PROXY: u16 = 11;
pub const IFLA_VXLAN_RSC: u16 = 12;
pub const IFLA_VXLAN_L2MISS: u16 = 13;
pub const IFLA_VXLAN_L3MISS: u16 = 14;
pub const IFLA_VXLAN_PORT: u16 = 15;
pub const IFLA_VXLAN_GROUP6: u16 = 16;
pub const IFLA_VXLAN_LOCAL6: u16 = 17;
pub const IFLA_VXLAN_UDP_CSUM: u16 = 18;
pub const IFLA_VXLAN_UDP_ZERO_CSUM6_TX: u16 = 19;
pub const IFLA_VXLAN_UDP_ZERO_CSUM6_RX: u16 = 20;
pub const IFLA_VXLAN_REMCSUM_TX: u16 = 21;
pub const IFLA_VXLAN_REMCSUM_RX: u16 = 22;
pub const IFLA_VXLAN_GBP: u16 = 23;
pub const IFLA_VXLAN_REMCSUM_NOPARTIAL: u16 = 24;
pub const IFLA_VXLAN_FLOWBASED: u16 = 25;
pub const IFLA_VXLAN_MAX: u16 = IFLA_VXLAN_FLOWBASED;

#[derive(Error, Debug)]
pub enum RouteError {
    #[error("invalid address length")]
    InvalidLength,
}

pub fn vec_to_addr(vec: &[u8]) -> Result<IpAddr> {
    match vec.len() {
        4 => {
            let buf: [u8; 4] = vec.try_into().unwrap();
            Ok(IpAddr::from(buf))
        }
        16 => {
            let buf: [u8; 16] = vec.try_into().unwrap();
            Ok(IpAddr::from(buf))
        }
        _ => Err(RouteError::InvalidLength.into()),
    }
}
