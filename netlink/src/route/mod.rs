use std::net::IpAddr;

use anyhow::Result;
use thiserror::Error;

pub mod addr;
pub mod link;
pub mod message;
pub mod routing;

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
