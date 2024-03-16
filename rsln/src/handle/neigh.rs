use std::{
    net::IpAddr,
    ops::{Deref, DerefMut},
};

use anyhow::{anyhow, Result};

use crate::{
    core::message::Message,
    types::{
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

        let neigh_msg = NeighborMessage::new(
            family,
            neigh.link_index,
            neigh.state,
            neigh.flags,
            neigh.neigh_type,
        );

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
    use crate::{
        parse_mac, test_setup,
        types::{
            link::{Kind, LinkAttrs},
            neigh::NeighborBuilder,
        },
    };

    use super::*;

    #[test]
    fn test_neigh_handle() {
        test_setup!();
        let mut handle = SocketHandle::new(libc::NETLINK_ROUTE);

        let mut link_handle = handle.handle_link();
        let attr = LinkAttrs::new("foo");

        let link = Kind::Dummy(attr.clone());

        link_handle
            .add(
                &link,
                libc::NLM_F_CREATE | libc::NLM_F_EXCL | libc::NLM_F_ACK,
            )
            .unwrap();

        let link = link_handle.get(&attr).unwrap();

        let mut neigh_handle = handle.handle_neigh();

        let mac_bytes = parse_mac("aa:bb:cc:dd:00:01").unwrap();

        let neigh = NeighborBuilder::default()
            .link_index(link.attrs().index as u32)
            .state(libc::NUD_PERMANENT)
            .neigh_type(libc::RTN_UNICAST)
            .ip_addr(Some(IpAddr::V4("10.244.0.0".parse().unwrap())))
            .mac_addr(Some(mac_bytes))
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
