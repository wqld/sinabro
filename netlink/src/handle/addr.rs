use std::{
    net::IpAddr,
    ops::{Deref, DerefMut},
};

use anyhow::Result;
use ipnet::IpNet;

use crate::{
    core::message::Message,
    route::{
        addr::Address,
        link::Link,
        message::{AddressMessage, Attribute, RouteAttr},
    },
};

use super::{sock_handle::SocketHandle, zero_terminated};

pub struct AddrHandle(SocketHandle);

impl Deref for AddrHandle {
    type Target = SocketHandle;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AddrHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<SocketHandle> for AddrHandle {
    fn from(handle: SocketHandle) -> Self {
        Self(handle)
    }
}

impl AddrHandle {
    pub fn handle<T>(&mut self, link: &T, addr: &Address, proto: u16, flags: i32) -> Result<()>
    where
        T: Link + ?Sized,
    {
        let mut req = Message::new(proto, flags);
        let base = link.attrs();
        let mut index: i32 = base.index;

        if index == 0 {
            let mut link_handle = self.handle_link();
            index = match link_handle.get(base) {
                Ok(link) => link.attrs().index,
                Err(_) => 0,
            }
        }

        let (family, local_addr_data) = match addr.ip {
            IpNet::V4(ip) => (libc::AF_INET, ip.addr().octets().to_vec()),
            IpNet::V6(ip) => (libc::AF_INET6, ip.addr().octets().to_vec()),
        };

        let peer_addr_data = match addr.peer {
            Some(IpNet::V4(ip)) if family == libc::AF_INET6 => {
                ip.addr().to_ipv6_mapped().octets().to_vec()
            }
            Some(IpNet::V6(ip)) if family == libc::AF_INET => {
                ip.addr().to_ipv4().unwrap().octets().to_vec()
            }
            Some(IpNet::V4(ip)) => ip.addr().octets().to_vec(),
            Some(IpNet::V6(ip)) => ip.addr().octets().to_vec(),
            None => local_addr_data.clone(),
        };

        let msg = Box::new(AddressMessage {
            family: family as u8,
            prefix_len: addr.ip.prefix_len(),
            flags: addr.flags,
            scope: addr.scope,
            index,
        });

        let local_data = RouteAttr::new(libc::IFA_LOCAL, &local_addr_data);
        let address_data = RouteAttr::new(libc::IFA_ADDRESS, &peer_addr_data);

        req.add(&msg.serialize()?);
        req.add(&local_data.serialize()?);
        req.add(&address_data.serialize()?);

        if family == libc::AF_INET {
            let broadcast = match addr.broadcast {
                Some(IpAddr::V4(br)) => br.octets().to_vec(),
                Some(IpAddr::V6(br)) => br.octets().to_vec(),
                None => match addr.ip.broadcast() {
                    IpAddr::V4(br) => br.octets().to_vec(),
                    IpAddr::V6(br) => br.octets().to_vec(),
                },
            };

            let broadcast_data = RouteAttr::new(libc::IFA_BROADCAST, &broadcast);
            req.add(&broadcast_data.serialize()?);

            if !addr.label.is_empty() {
                let label_data = RouteAttr::new(libc::IFA_LABEL, &zero_terminated(&addr.label));
                req.add(&label_data.serialize()?);
            }
        }

        self.request(&mut req, 0)?;

        Ok(())
    }

    pub fn show<T>(&mut self, link: &T, family: i32) -> Result<Vec<Address>>
    where
        T: Link + ?Sized,
    {
        let link_index = link.attrs().index;
        let mut req = Message::new(libc::RTM_GETADDR, libc::NLM_F_DUMP);
        let msg = AddressMessage::new(family);
        req.add(&msg.serialize()?);

        Ok(self
            .request(&mut req, libc::RTM_NEWADDR)?
            .iter()
            .filter_map(|m| {
                let addr = Address::from(m.as_slice());
                if addr.index == link_index {
                    Some(addr)
                } else {
                    None
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        route::{addr::Address, link::LinkAttrs},
        test_setup,
    };

    #[test]
    fn test_addr_handle() {
        test_setup!();
        let handle = super::SocketHandle::new(libc::NETLINK_ROUTE);
        let mut link_handle = handle.handle_link();
        let mut addr_handle = handle.handle_addr();
        let mut attr = LinkAttrs::new();
        attr.name = "lo".to_string();

        let link = link_handle.get(&attr).unwrap();

        let address = "127.0.0.2/24".parse().unwrap();
        let addr = Address {
            ip: address,
            ..Default::default()
        };

        let proto = libc::RTM_NEWADDR;
        let flags = libc::NLM_F_CREATE | libc::NLM_F_EXCL | libc::NLM_F_ACK;

        addr_handle.handle(&link, &addr, proto, flags).unwrap();

        let addrs = addr_handle.show(&link, libc::AF_UNSPEC).unwrap();

        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0].ip, address);
    }
}
