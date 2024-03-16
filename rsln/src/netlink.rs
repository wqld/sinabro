use std::collections::HashMap;

use anyhow::Result;
use sysctl::Sysctl;

use crate::{
    handle::sock_handle::SocketHandle,
    types::{
        addr::{AddrCmd, AddrFamily, Address},
        generic::{GenlFamilies, GenlFamily},
        link::{Link, LinkAttrs},
        neigh::Neighbor,
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

    pub fn ensure_link<T: Link + ?Sized>(&mut self, link: &T) -> Result<Box<dyn Link>> {
        let link = self.link_get(link.attrs()).or_else(|_| {
            self.link_add(link)?;
            self.link_get(link.attrs())
        })?;

        self.enable_forwarding(&link, true, true)?;
        Ok(link)
    }

    pub fn enable_forwarding<T: Link + ?Sized>(
        &mut self,
        link: &T,
        enable_ipv6: bool,
        enable_ipv4: bool,
    ) -> Result<()> {
        self.link_up(link)?;

        let if_name = &link.attrs().name;
        let mut sys_settings = Vec::new();

        if enable_ipv6 {
            sys_settings.push((format!("net.ipv6.conf.{}.forwarding", if_name), "1"));
        }

        if enable_ipv4 {
            sys_settings.push((format!("net.ipv4.conf.{}.forwarding", if_name), "1"));
            sys_settings.push((format!("net.ipv4.conf.{}.rp_filter", if_name), "0"));
            sys_settings.push((format!("net.ipv4.conf.{}.accept_local", if_name), "1"));
            sys_settings.push((format!("net.ipv4.conf.{}.send_redirects", if_name), "0"));
        }

        for setting in sys_settings {
            let ctl = sysctl::Ctl::new(&setting.0)?;
            ctl.set_value_string(setting.1)?;
        }

        Ok(())
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

    pub fn neigh_set(&mut self, neigh: &Neighbor) -> Result<()> {
        self.sockets
            .entry(libc::NETLINK_ROUTE)
            .or_insert(SocketHandle::new(libc::NETLINK_ROUTE))
            .handle_neigh()
            .handle(
                neigh,
                libc::RTM_NEWNEIGH,
                libc::NLM_F_CREATE | libc::NLM_F_REPLACE | libc::NLM_F_ACK,
            )
    }

    pub fn genl_family_list(&mut self) -> Result<GenlFamilies> {
        self.sockets
            .entry(libc::NETLINK_GENERIC)
            .or_insert(SocketHandle::new(libc::NETLINK_GENERIC))
            .handle_generic()
            .list_family()
    }

    pub fn genl_family_get(&mut self, name: &str) -> Result<GenlFamily> {
        self.sockets
            .entry(libc::NETLINK_GENERIC)
            .or_insert(SocketHandle::new(libc::NETLINK_GENERIC))
            .handle_generic()
            .get_family(name)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        test_setup,
        types::link::{Kind, VxlanAttrs},
    };

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

    #[test]
    fn test_ensure_link() {
        test_setup!();
        let mut netlink = Netlink::new();
        let vxlan_mac = vec![0x02, 0x1A, 0x79, 0x35, 0x1C, 0x5D];
        let link = Kind::Vxlan {
            attrs: LinkAttrs {
                name: "sinabro_vxlan".to_string(),
                mtu: 1500,
                hw_addr: vxlan_mac,
                ..Default::default()
            },
            vxlan_attrs: VxlanAttrs {
                flow_based: true,
                port: Some(8472),
                ..Default::default()
            },
        };
        let link = netlink.ensure_link(&link);

        assert!(link.is_ok());
        println!("{:?}", link.unwrap().kind());
    }
}
