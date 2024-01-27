use std::{
    net::IpAddr,
    ops::{Deref, DerefMut},
};

use anyhow::{bail, Result};
use ipnet::IpNet;

use crate::{
    core::message::Message,
    route::{
        message::{Attribute, RouteAttr, RouteMessage},
        routing::Routing,
    },
};

use super::sock_handle::SocketHandle;

pub struct RouteHandle(SocketHandle);

impl Deref for RouteHandle {
    type Target = SocketHandle;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for RouteHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<SocketHandle> for RouteHandle {
    fn from(handle: SocketHandle) -> Self {
        Self(handle)
    }
}

impl RouteHandle {
    pub fn handle(&mut self, route: &Routing, proto: u16, flags: i32) -> Result<()> {
        let mut req = Message::new(proto, flags);

        let mut msg = match proto {
            libc::RTM_DELROUTE => Box::new(RouteMessage::new_delete_msg()),
            _ => Box::new(RouteMessage::new()),
        };

        let mut attrs = vec![];

        if proto != libc::RTM_GETROUTE || route.oif_index > 0 {
            let mut b = [0; 4];
            b.copy_from_slice(&route.oif_index.to_ne_bytes());
            attrs.push(RouteAttr::new(libc::RTA_OIF, &b));
        }

        if let Some(dst) = route.dst {
            let (family, dst_data) = match dst {
                IpNet::V4(ip) => (libc::AF_INET, ip.addr().octets().to_vec()),
                IpNet::V6(ip) => (libc::AF_INET6, ip.addr().octets().to_vec()),
            };
            msg.family = family as u8;
            msg.dst_len = dst.prefix_len();

            attrs.push(RouteAttr::new(libc::RTA_DST, &dst_data));
        }

        if let Some(src) = route.src {
            let (family, src_data) = match src {
                IpAddr::V4(ip) => (libc::AF_INET, ip.octets().to_vec()),
                IpAddr::V6(ip) => (libc::AF_INET6, ip.octets().to_vec()),
            };

            if msg.family == 0 {
                msg.family = family as u8;
            } else if msg.family != family as u8 {
                bail!("src and dst address family mismatch");
            }

            attrs.push(RouteAttr::new(libc::RTA_PREFSRC, &src_data));
        }

        if let Some(gw) = route.gw {
            let (family, gw_data) = match gw {
                IpAddr::V4(ip) => (libc::AF_INET, ip.octets().to_vec()),
                IpAddr::V6(ip) => (libc::AF_INET6, ip.octets().to_vec()),
            };

            if msg.family == 0 {
                msg.family = family as u8;
            } else if msg.family != family as u8 {
                bail!("gw, src and dst address family mismatch");
            }

            attrs.push(RouteAttr::new(libc::RTA_GATEWAY, &gw_data));
        }

        // TODO: more attributes to be added

        msg.flags = route.flags;
        msg.scope = route.scope;

        req.add(&msg.serialize()?);

        for attr in attrs {
            req.add(&attr.serialize()?);
        }

        self.request(&mut req, 0)?;

        Ok(())
    }

    pub fn get(&mut self, dst: &IpAddr) -> Result<Vec<Routing>> {
        let mut req = Message::new(libc::RTM_GETROUTE, libc::NLM_F_REQUEST);
        let (family, dst_data, bit_len) = match dst {
            IpAddr::V4(ip) => (libc::AF_INET, ip.octets().to_vec(), 32),
            IpAddr::V6(ip) => (libc::AF_INET6, ip.octets().to_vec(), 128),
        };

        let mut msg = RouteMessage {
            ..Default::default()
        };

        msg.family = family as u8;
        msg.dst_len = bit_len;
        msg.flags = libc::RTM_F_LOOKUP_TABLE;

        let rta_dst = RouteAttr::new(libc::RTA_DST, &dst_data);

        req.add(&msg.serialize()?);
        req.add(&rta_dst.serialize()?);

        Ok(self
            .request(&mut req, libc::RTM_NEWROUTE)?
            .into_iter()
            .map(|m| Routing::from(m.as_slice()))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::{route::link::LinkAttrs, test_setup};

    use super::*;

    #[test]
    fn test_route_handle() {
        test_setup!();
        let handle = super::SocketHandle::new(libc::NETLINK_ROUTE);
        let mut link_handle = handle.handle_link();
        let mut route_handle = handle.handle_route();
        let mut attr = LinkAttrs::new();
        attr.name = "lo".to_string();

        let link = link_handle.get(&attr).unwrap();

        link_handle.setup(&link).unwrap();

        let route = Routing {
            oif_index: link.attrs().index,
            dst: Some("192.168.0.0/24".parse().unwrap()),
            src: Some("127.0.0.2".parse().unwrap()),
            ..Default::default()
        };

        route_handle
            .handle(
                &route,
                libc::RTM_NEWROUTE,
                libc::NLM_F_CREATE | libc::NLM_F_EXCL | libc::NLM_F_ACK,
            )
            .unwrap();

        let routes = route_handle.get(&route.dst.unwrap().addr()).unwrap();

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].oif_index, link.attrs().index);
        assert_eq!(
            routes[0].dst.unwrap().network(),
            route.dst.unwrap().network()
        );

        route_handle
            .handle(&route, libc::RTM_DELROUTE, libc::NLM_F_ACK)
            .unwrap();

        let res = route_handle.get(&route.dst.unwrap().addr()).err();
        assert!(res.is_some());
    }
}
