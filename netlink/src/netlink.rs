use std::collections::HashMap;

use anyhow::Result;

use crate::{
    handle::sock_handle::SocketHandle,
    route::{
        addr::{AddrCmd, AddrFamily, Address},
        link::{Link, LinkAttrs},
        routing::{Routing, RtCmd},
    },
};

#[derive(Default)]
pub struct Netlink {
    pub sockets: HashMap<i32, SocketHandle>,
}

impl Netlink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn link_get(&mut self, attr: &LinkAttrs) -> Result<Box<dyn Link>> {
        self.sockets
            .entry(libc::NETLINK_ROUTE)
            .or_insert(SocketHandle::new(libc::NETLINK_ROUTE))
            .handle_link()
            .get(attr)
    }

    pub fn link_add<T: Link + ?Sized>(&mut self, link: &T) -> Result<()> {
        let flags = libc::NLM_F_CREATE | libc::NLM_F_EXCL | libc::NLM_F_ACK;
        self.sockets
            .entry(libc::NETLINK_ROUTE)
            .or_insert(SocketHandle::new(libc::NETLINK_ROUTE))
            .handle_link()
            .add(link, flags)
    }

    pub fn link_up<T: Link + ?Sized>(&mut self, link: &T) -> Result<()> {
        self.sockets
            .entry(libc::NETLINK_ROUTE)
            .or_insert(SocketHandle::new(libc::NETLINK_ROUTE))
            .handle_link()
            .up(link)
    }

    pub fn link_set_master<T: Link + ?Sized>(&mut self, link: &T, master_index: i32) -> Result<()> {
        self.sockets
            .entry(libc::NETLINK_ROUTE)
            .or_insert(SocketHandle::new(libc::NETLINK_ROUTE))
            .handle_link()
            .set_master(link, master_index)
    }

    pub fn link_set_ns<T: Link + ?Sized>(&mut self, link: &T, ns: i32) -> Result<()> {
        self.sockets
            .entry(libc::NETLINK_ROUTE)
            .or_insert(SocketHandle::new(libc::NETLINK_ROUTE))
            .handle_link()
            .set_ns(link, ns)
    }

    pub fn link_set_name<T: Link + ?Sized>(&mut self, link: &T, name: &str) -> Result<()> {
        self.sockets
            .entry(libc::NETLINK_ROUTE)
            .or_insert(SocketHandle::new(libc::NETLINK_ROUTE))
            .handle_link()
            .set_name(link, name)
    }

    pub fn addr_list(
        &mut self,
        link: &(impl Link + ?Sized),
        family: AddrFamily,
    ) -> Result<Vec<Address>> {
        self.sockets
            .entry(libc::NETLINK_ROUTE)
            .or_insert(SocketHandle::new(libc::NETLINK_ROUTE))
            .handle_addr()
            .list(link, family.into())
    }

    pub fn addr_add(&mut self, link: &(impl Link + ?Sized), addr: &Address) -> Result<()> {
        self.addr_handle(AddrCmd::Add, link, addr)
    }

    pub fn addr_replace(&mut self, link: &(impl Link + ?Sized), addr: &Address) -> Result<()> {
        self.addr_handle(AddrCmd::Replace, link, addr)
    }

    pub fn addr_del(&mut self, link: &(impl Link + ?Sized), addr: &Address) -> Result<()> {
        self.addr_handle(AddrCmd::Delete, link, addr)
    }

    fn addr_handle(
        &mut self,
        command: AddrCmd,
        link: &(impl Link + ?Sized),
        addr: &Address,
    ) -> Result<()> {
        let (proto, flags) = match command {
            AddrCmd::Add => (
                libc::RTM_NEWADDR,
                libc::NLM_F_CREATE | libc::NLM_F_EXCL | libc::NLM_F_ACK,
            ),
            AddrCmd::Change => (libc::RTM_NEWADDR, libc::NLM_F_REPLACE | libc::NLM_F_ACK),
            AddrCmd::Replace => (
                libc::RTM_NEWADDR,
                libc::NLM_F_CREATE | libc::NLM_F_REPLACE | libc::NLM_F_ACK,
            ),
            AddrCmd::Delete => (libc::RTM_DELADDR, libc::NLM_F_ACK),
        };

        self.sockets
            .entry(libc::NETLINK_ROUTE)
            .or_insert(SocketHandle::new(libc::NETLINK_ROUTE))
            .handle_addr()
            .handle(link, addr, proto, flags)
    }

    pub fn route_add(&mut self, route: &Routing) -> Result<()> {
        self.route_handle(RtCmd::Add, route)
    }

    fn route_handle(&mut self, cmd: RtCmd, route: &Routing) -> Result<()> {
        let (proto, flags) = match cmd {
            RtCmd::Add => (
                libc::RTM_NEWROUTE,
                libc::NLM_F_CREATE | libc::NLM_F_EXCL | libc::NLM_F_ACK,
            ),
            RtCmd::Append => (
                libc::RTM_NEWROUTE,
                libc::NLM_F_CREATE | libc::NLM_F_APPEND | libc::NLM_F_ACK,
            ),
            RtCmd::Replace => (
                libc::RTM_NEWROUTE,
                libc::NLM_F_CREATE | libc::NLM_F_REPLACE | libc::NLM_F_ACK,
            ),
            RtCmd::Delete => (libc::RTM_DELROUTE, libc::NLM_F_ACK),
        };

        self.sockets
            .entry(libc::NETLINK_ROUTE)
            .or_insert(SocketHandle::new(libc::NETLINK_ROUTE))
            .handle_route()
            .handle(route, proto, flags)
    }
}

#[cfg(test)]
mod tests {
    use crate::{route::link::Kind, test_setup};

    use super::*;

    #[test]
    fn test_setup_bridge() {
        test_setup!();
        let mut netlink = Netlink::new();

        let link = Kind::new_bridge("foo");

        netlink.link_add(&link).unwrap();

        let link = netlink.link_get(&LinkAttrs::new("foo")).unwrap();

        netlink.link_up(&link).unwrap();

        let link = netlink.link_get(&LinkAttrs::new("foo")).unwrap();
        assert_ne!(link.attrs().oper_state, 2);
    }
}
