use std::mem::size_of;

use crate::{consts, utils::align_of};

pub struct NetlinkMessage {
    pub header: NetlinkHeader,
    pub payload: Vec<u8>,
    pub len: usize,
}

impl<'a> From<&'a [u8]> for NetlinkMessage {
    fn from(buf: &'a [u8]) -> Self {
        let header: NetlinkHeader = buf.into();
        let len = align_of(header.len as usize, consts::NLMSG_ALIGN_TO);
        let payload = buf[consts::NLMSG_HDR_LEN..len].to_vec();

        Self {
            header,
            payload,
            len,
        }
    }
}

pub struct NetlinkMessages(Vec<NetlinkMessage>);

impl<'a> From<&'a [u8]> for NetlinkMessages {
    fn from(buf: &'a [u8]) -> Self {
        let mut buf = buf;
        let mut req = Vec::new();

        while buf.len() >= consts::NLMSG_HDR_LEN {
            let msg: NetlinkMessage = buf.into();
            buf = &buf[msg.len..];
            req.push(msg);
        }

        Self(req)
    }
}

impl IntoIterator for NetlinkMessages {
    type Item = NetlinkMessage;
    type IntoIter = <Vec<NetlinkMessage> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct NetlinkHeader {
    pub len: u32,
    pub kind: u16,
    pub flags: u16,
    pub seq: u32,
    pub pid: u32,
}

impl<'a> From<&'a [u8]> for NetlinkHeader {
    fn from(buf: &'a [u8]) -> Self {
        unsafe { *(buf.as_ptr() as *const Self) }
    }
}

impl NetlinkHeader {
    pub fn new(proto: u16, flags: i32) -> Self {
        Self {
            len: size_of::<Self>() as u32,
            kind: proto,
            flags: flags as u16,
            ..Default::default()
        }
    }
}
