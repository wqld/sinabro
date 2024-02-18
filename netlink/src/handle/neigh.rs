use std::{
    net::IpAddr,
    ops::{Deref, DerefMut},
};

use anyhow::{anyhow, Result};

use crate::{
    core::message::Message,
    route::{
        message::{Attribute, NeighborMessage, RouteAttr},
        neigh::Neighbor,
    },
};

use super::sock_handle::SocketHandle;

pub struct NeighHandle<'a> {
    pub socket: &'a mut SocketHandle,
}

impl<'a> Deref for NeighHandle<'a> {
    type Target = SocketHandle;

    fn deref(&self) -> &Self::Target {
        self.socket
    }
}

impl<'a> DerefMut for NeighHandle<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.socket
    }
}

impl<'a> From<&'a mut SocketHandle> for NeighHandle<'a> {
    fn from(socket: &'a mut SocketHandle) -> Self {
        Self { socket }
    }
}

impl NeighHandle<'_> {
    pub fn handle(&mut self, neigh: &Neighbor, proto: u16, flags: i32) -> Result<()> {
        let mut req = Message::new(proto, flags);

        let (family, ip_addr_vec) = match neigh.ip_addr {
            Some(IpAddr::V4(ip)) => (libc::AF_INET as u8, ip.octets().to_vec()),
            Some(IpAddr::V6(ip)) => (libc::AF_INET6 as u8, ip.octets().to_vec()),
            None => return Err(anyhow!("IP address is required")),
        };

        let family = neigh.family.map_or(family, |f| f);

        let neigh_msg = NeighborMessage {
            family,
            index: neigh.link_index,
            state: neigh.state,
            flags: neigh.flags,
            neigh_type: neigh.neigh_type,
        };

        let destination = RouteAttr::new(libc::NDA_DST, &ip_addr_vec);

        req.add(&neigh_msg.serialize()?);
        req.add(&destination.serialize()?);

        if let Some(mac_addr) = &neigh.mac_addr {
            let mac = RouteAttr::new(libc::NDA_LLADDR, mac_addr);
            req.add(&mac.serialize()?);
        }

        self.request(&mut req, 0)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{route::neigh::NeighborBuilder, test_setup};

    use super::*;

    #[test]
    fn test_neigh_handle() {
        test_setup!();
        let mut handle = SocketHandle::new(libc::NETLINK_ROUTE);
        let mut neigh_handle = handle.handle_neigh();

        let neigh = NeighborBuilder::default()
            .link_index(5)
            .state(128)
            .ip_addr(Some(IpAddr::V4("10.244.1.0".parse().unwrap())))
            .mac_addr(Some(vec![0x02, 0x12, 0x34, 0x56, 0x78, 0x9A]))
            .neigh_type(1)
            .build()
            .unwrap();

        neigh_handle
            .handle(
                &neigh,
                libc::RTM_NEWNEIGH,
                libc::NLM_F_CREATE | libc::NLM_F_REPLACE | libc::NLM_F_ACK,
            )
            .unwrap();
    }
}
