use std::ops::{Deref, DerefMut};

use anyhow::{bail, Result};

use crate::{
    core::message::Message,
    route::{
        link::{Kind, Link, LinkAttrs},
        message::{Attribute, LinkMessage, RouteAttr},
    },
};

use super::{sock_handle::SocketHandle, zero_terminated};

const IFF_UP: u32 = 0x1;

pub struct LinkHandle<'a> {
    pub socket: &'a mut SocketHandle,
}

impl<'a> Deref for LinkHandle<'a> {
    type Target = SocketHandle;

    fn deref(&self) -> &Self::Target {
        self.socket
    }
}

impl DerefMut for LinkHandle<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.socket
    }
}

impl<'a> From<&'a mut SocketHandle> for LinkHandle<'a> {
    fn from(socket: &'a mut SocketHandle) -> Self {
        Self { socket }
    }
}

impl LinkHandle<'_> {
    pub fn add<T: Link + ?Sized>(&mut self, link: &T, flags: i32) -> Result<()> {
        let base = link.attrs();
        let mut req = Message::new(libc::RTM_NEWLINK, flags);
        let mut msg = LinkMessage::new(libc::AF_UNSPEC);

        if base.index != 0 {
            msg.index = base.index;
        }

        if base.flags & IFF_UP != 0 {
            msg.flags = IFF_UP;
            msg.change_mask = IFF_UP;
        }

        req.add(&msg.serialize()?);

        let name = RouteAttr::new(libc::IFLA_IFNAME, &zero_terminated(&base.name));
        req.add(&name.serialize()?);

        if base.mtu > 0 {
            let attr = RouteAttr::new(libc::IFLA_MTU, &base.mtu.to_ne_bytes());
            req.add(&attr.serialize()?);
        }

        if base.tx_queue_len > 0 {
            let attr = RouteAttr::new(libc::IFLA_TXQLEN, &base.tx_queue_len.to_ne_bytes());
            req.add(&attr.serialize()?);
        }

        if base.num_tx_queues > 0 {
            let attr = RouteAttr::new(libc::IFLA_NUM_TX_QUEUES, &base.num_tx_queues.to_ne_bytes());
            req.add(&attr.serialize()?);
        }

        if base.num_rx_queues > 0 {
            let attr = RouteAttr::new(libc::IFLA_NUM_RX_QUEUES, &base.num_rx_queues.to_ne_bytes());
            req.add(&attr.serialize()?);
        }

        let mut link_info = RouteAttr::new(libc::IFLA_LINKINFO, &[]);

        link_info.add(libc::IFLA_INFO_KIND, link.link_type().as_bytes());

        let opt_attr: Option<RouteAttr> = Option::from(link.kind());
        if let Some(link_attr) = opt_attr {
            link_info.add_attribute(Box::new(link_attr));
        }

        req.add(&link_info.serialize()?);

        self.request(&mut req, 0)?;

        Ok(())
    }

    pub fn delete<T: Link + ?Sized>(&mut self, link: &T) -> Result<()> {
        let base = link.attrs();

        let mut req = Message::new(libc::RTM_DELLINK, libc::NLM_F_ACK);

        let mut msg = LinkMessage::new(libc::AF_UNSPEC);
        msg.index = base.index;

        req.add(&msg.serialize()?);

        self.request(&mut req, 0)?;

        Ok(())
    }

    pub fn get(&mut self, attr: &LinkAttrs) -> Result<Box<dyn Link>> {
        let mut req = Message::new(libc::RTM_GETLINK, libc::NLM_F_ACK);
        let mut msg = LinkMessage::new(libc::AF_UNSPEC);

        if attr.index != 0 {
            msg.index = attr.index;
        }

        req.add(&msg.serialize()?);

        if !attr.name.is_empty() {
            let n = attr.name.clone();
            let name = RouteAttr::new(libc::IFLA_IFNAME, n.as_bytes());
            req.add(&name.serialize()?);
        }

        let msgs = self.request(&mut req, 0)?;

        match msgs.len() {
            0 => bail!("no link found"),
            1 => {
                let msg = Kind::from(msgs[0].as_slice());
                Ok(Box::new(msg))
            }
            _ => bail!("multiple links found"),
        }
    }

    pub fn up<T: Link + ?Sized>(&mut self, link: &T) -> Result<()> {
        let mut req = Message::new(libc::RTM_NEWLINK, libc::NLM_F_ACK);
        let base = link.attrs();

        let mut msg = Box::new(LinkMessage::new(libc::AF_UNSPEC));
        msg.index = base.index;
        msg.flags = libc::IFF_UP as u32;
        msg.change_mask = libc::IFF_UP as u32;

        req.add(&msg.serialize()?);

        self.request(&mut req, 0)?;

        Ok(())
    }

    pub fn set_master<T: Link + ?Sized>(&mut self, link: &T, master_index: i32) -> Result<()> {
        let mut req = Message::new(libc::RTM_SETLINK, libc::NLM_F_ACK);
        let base = link.attrs();

        let mut msg = Box::new(LinkMessage::new(libc::AF_UNSPEC));
        msg.index = base.index;

        let master_attr = RouteAttr::new(libc::IFLA_MASTER, &master_index.to_ne_bytes());

        req.add(&msg.serialize()?);
        req.add(&master_attr.serialize()?);

        self.request(&mut req, 0)?;

        Ok(())
    }

    pub fn set_ns<T: Link + ?Sized>(&mut self, link: &T, ns: i32) -> Result<()> {
        let mut req = Message::new(libc::RTM_SETLINK, libc::NLM_F_ACK);
        let base = link.attrs();

        let mut msg = Box::new(LinkMessage::new(libc::AF_UNSPEC));
        msg.index = base.index;

        let ns_attr = RouteAttr::new(libc::IFLA_NET_NS_FD, &ns.to_ne_bytes());

        req.add(&msg.serialize()?);
        req.add(&ns_attr.serialize()?);

        self.request(&mut req, 0)?;

        Ok(())
    }

    pub fn set_name<T: Link + ?Sized>(&mut self, link: &T, name: &str) -> Result<()> {
        let mut req = Message::new(libc::RTM_SETLINK, libc::NLM_F_ACK);
        let base = link.attrs();

        let mut msg = Box::new(LinkMessage::new(libc::AF_UNSPEC));
        msg.index = base.index;

        let name_attr = RouteAttr::new(libc::IFLA_IFNAME, name.as_bytes());

        req.add(&msg.serialize()?);
        req.add(&name_attr.serialize()?);

        self.request(&mut req, 0)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        handle::sock_handle,
        route::link::{Kind, LinkAttrs},
        test_setup,
    };

    #[tokio::test]
    async fn test_link_add_modify_del() {
        test_setup!();
        let mut handle = sock_handle::SocketHandle::new(libc::NETLINK_ROUTE);
        let mut link_handle = handle.handle_link();
        let mut attr = LinkAttrs::new("foo");

        let link = Kind::Dummy(attr.clone());

        link_handle
            .add(
                &link,
                libc::NLM_F_CREATE | libc::NLM_F_EXCL | libc::NLM_F_ACK,
            )
            .unwrap();

        let link = link_handle.get(&attr).unwrap();
        assert_eq!(link.attrs().name, "foo");

        attr = link.attrs().clone();
        attr.name = "bar".to_string();

        let link = Kind::Dummy(attr.clone());

        link_handle.add(&link, libc::NLM_F_ACK).unwrap();

        let link = link_handle.get(&attr).unwrap();
        assert_eq!(link.attrs().name, "bar");

        link_handle.delete(&link).unwrap();

        let res = link_handle.get(&attr).err();
        println!("{res:?}");
        assert!(res.is_some());
    }

    #[test]
    fn test_link_bridge() {
        test_setup!();
        let mut handle = sock_handle::SocketHandle::new(libc::NETLINK_ROUTE);
        let mut link_handle = handle.handle_link();
        let attr = LinkAttrs::new("foo");

        let link = Kind::Bridge {
            attrs: attr.clone(),
            hello_time: None,
            ageing_time: Some(30102),
            vlan_filtering: Some(true),
            multicast_snooping: None,
        };

        link_handle
            .add(
                &link,
                libc::NLM_F_CREATE | libc::NLM_F_EXCL | libc::NLM_F_ACK,
            )
            .unwrap();

        let link = link_handle.get(&attr).unwrap();
        assert_eq!(link.attrs().link_type, "bridge");
        assert_eq!(link.attrs().name, "foo");

        match link.kind() {
            Kind::Bridge {
                attrs: _,
                hello_time,
                ageing_time,
                vlan_filtering,
                multicast_snooping,
            } => {
                assert_eq!(hello_time.unwrap(), 200);
                assert_eq!(ageing_time.unwrap(), 30102);
                assert!(vlan_filtering.unwrap());
                assert!(multicast_snooping.unwrap());
            }
            _ => panic!("wrong link type"),
        }

        link_handle.delete(&link).unwrap();

        let res = link_handle.get(&attr).err();
        assert!(res.is_some());
    }

    #[test]
    fn test_link_veth() {
        test_setup!();
        let mut handle = sock_handle::SocketHandle::new(libc::NETLINK_ROUTE);
        let mut link_handle = handle.handle_link();
        let mut attr = LinkAttrs::new("foo");
        attr.mtu = 1400;
        attr.tx_queue_len = 100;
        attr.num_tx_queues = 4;
        attr.num_rx_queues = 8;

        // TODO: need to set peer hw addr and peer ns
        let link = Kind::Veth {
            attrs: attr.clone(),
            peer_name: "bar".to_string(),
            peer_hw_addr: None,
            peer_ns: None,
        };

        link_handle
            .add(
                &link,
                libc::NLM_F_CREATE | libc::NLM_F_EXCL | libc::NLM_F_ACK,
            )
            .unwrap();

        let link = link_handle.get(&attr).unwrap();

        let peer = link_handle
            .get(&LinkAttrs {
                name: "bar".to_string(),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(link.attrs().link_type, "veth");
        assert_eq!(link.attrs().name, "foo");
        assert_eq!(link.attrs().mtu, 1400);
        assert_eq!(link.attrs().tx_queue_len, 100);
        assert_eq!(link.attrs().num_tx_queues, 4);
        assert_eq!(link.attrs().num_rx_queues, 8);

        assert_eq!(peer.attrs().link_type, "veth");
        assert_eq!(peer.attrs().name, "bar");
        assert_eq!(peer.attrs().mtu, 1400);
        assert_eq!(peer.attrs().tx_queue_len, 100);
        assert_eq!(peer.attrs().num_tx_queues, 4);
        assert_eq!(peer.attrs().num_rx_queues, 8);

        link_handle.delete(&peer).unwrap();

        let res = link_handle.get(&attr).err();
        assert!(res.is_some());
    }

    #[test]
    fn test_link_get() {
        test_setup!();
        let mut handle = sock_handle::SocketHandle::new(libc::NETLINK_ROUTE);
        let mut link_handle = handle.handle_link();
        let attr = LinkAttrs::new("lo");

        let link = link_handle.get(&attr).unwrap();

        assert_eq!(link.attrs().index, 1);
        assert_eq!(link.attrs().name, "lo");
    }
}
