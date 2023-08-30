use std::mem::size_of;

use anyhow::Result;

use crate::consts;

pub struct NetlinkMessage {
    pub header: NetlinkHeader,
    pub data: Vec<u8>,
}

impl NetlinkMessage {
    pub fn from(mut buf: &[u8]) -> Result<Vec<Self>> {
        let mut msgs = Vec::new();

        while buf.len() >= consts::NLMSG_HDR_LEN {
            let header = NetlinkHeader::from(buf);
            let len = align_of(header.len as usize, consts::NLMSG_ALIGN_TO);
            let data = buf[consts::NLMSG_HDR_LEN..len].to_vec();

            msgs.push(Self { header, data });
            buf = &buf[len..];
        }

        Ok(msgs)
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct NetlinkHeader {
    pub len: u32,
    pub ty: u16,
    pub flags: u16,
    pub seq: u32,
    pub pid: u32,
}

impl NetlinkHeader {
    pub fn new(proto: u16, flags: i32) -> Self {
        Self {
            len: size_of::<Self>() as u32,
            ty: proto,
            flags: flags as u16,
            ..Default::default()
        }
    }

    pub fn from(buf: &[u8]) -> Self {
        unsafe { *(buf.as_ptr() as *const Self) }
    }
}

pub fn align_of(len: usize, align_to: usize) -> usize {
    (len + align_to - 1) & !(align_to - 1)
}
