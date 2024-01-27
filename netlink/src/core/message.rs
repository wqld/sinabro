use std::{
    mem::size_of,
    ops::{ControlFlow, Deref, DerefMut},
    vec,
};

use anyhow::{anyhow, bail, Error, Ok, Result};
use libc::{NLM_F_MULTI, NLM_F_REQUEST};
use serde::{Deserialize, Serialize};

use crate::align_of;

const NLMSG_ALIGNTO: usize = 0x4;
const NLMSG_HDRLEN: usize = 0x10;

const NLMSG_DONE: u16 = 3;
const NLMSG_ERROR: u16 = 2;

pub struct Messages(Vec<Message>);

impl From<&[u8]> for Messages {
    fn from(mut buf: &[u8]) -> Self {
        let mut messages = Vec::new();

        while buf.len() >= NLMSG_HDRLEN {
            let message = Message::from(buf);
            let len = align_of(message.header.nlmsg_len as usize, NLMSG_ALIGNTO);
            messages.push(message);
            buf = &buf[len..];
        }

        Self(messages)
    }
}

impl IntoIterator for Messages {
    type Item = Message;
    type IntoIter = vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Deref for Messages {
    type Target = Vec<Message>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Messages {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Messages {}

pub struct Message {
    pub header: Header,
    pub payload: Option<Vec<u8>>,
}

impl From<&[u8]> for Message {
    fn from(buf: &[u8]) -> Self {
        let header: Header = bincode::deserialize(buf).expect("Failed to deserialize header");
        let data = buf[NLMSG_HDRLEN..header.nlmsg_len as usize].to_vec();
        Self {
            header,
            payload: Some(data),
        }
    }
}

impl Message {
    pub fn new(proto: u16, flags: i32) -> Self {
        Self {
            header: Header::new(proto, flags),
            payload: None,
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        let estimated_size = match &self.payload {
            Some(payload) => NLMSG_HDRLEN + payload.len(),
            None => NLMSG_HDRLEN,
        };

        let mut buf = Vec::with_capacity(estimated_size);
        buf.extend(bincode::serialize(&self.header)?);

        if let Some(payload) = &self.payload {
            buf.extend(payload);
        }

        let len = buf.len() as u16;
        buf[..2].copy_from_slice(&len.to_ne_bytes());

        Ok(buf)
    }

    pub fn add(&mut self, data: &[u8]) {
        self.header.nlmsg_len += data.len() as u32;
        let payload = self.payload.get_or_insert_with(Vec::new);
        payload.extend(data);
    }

    pub fn verify_header(&self, seq: u32, pid: u32) -> Result<()> {
        self.header.verify(seq, pid)
    }

    pub fn check_last_message(&self) -> bool {
        self.header.nlmsg_flags & NLM_F_MULTI as u16 == 0
    }
}

#[repr(C)]
#[derive(Serialize, Deserialize)]
pub struct Header {
    pub nlmsg_len: u32,
    pub nlmsg_type: u16,
    pub nlmsg_flags: u16,
    pub nlmsg_seq: u32,
    pub nlmsg_pid: u32,
}

impl Header {
    pub fn new(proto: u16, flags: i32) -> Self {
        Self {
            nlmsg_len: size_of::<Self>() as u32,
            nlmsg_type: proto,
            nlmsg_flags: (NLM_F_REQUEST | flags) as u16,
            nlmsg_seq: 0,
            nlmsg_pid: 0,
        }
    }

    pub fn verify(&self, seq: u32, pid: u32) -> Result<()> {
        if self.nlmsg_seq != seq {
            bail!("Invalid sequence number: {} != {}", self.nlmsg_seq, seq);
        }

        if self.nlmsg_pid != pid {
            bail!("Invalid process ID: {} != {}", self.nlmsg_pid, pid);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::route::message::{Attribute, RouteAttr};

    use super::*;

    #[test]
    fn test_messages_from_bytes() {
        let buf: [u8; 32] = [
            // First message
            0x10, 0x00, 0x00, 0x00, // nlmsg_len = 16
            0x00, 0x10, // nlmsg_type = 16
            0x01, 0x00, // nlmsg_flags = 1
            0x01, 0x00, 0x00, 0x00, // nlmsg_seq = 1
            0x01, 0x00, 0x00, 0x00, // nlmsg_pid = 1
            // Second message
            0x10, 0x00, 0x00, 0x00, // nlmsg_len = 16
            0x00, 0x10, // nlmsg_type = 16
            0x01, 0x00, // nlmsg_flags = 1
            0x02, 0x00, 0x00, 0x00, // nlmsg_seq = 2
            0x01, 0x00, 0x00, 0x00, // nlmsg_pid = 1
        ];

        let messages = Messages::from(&buf[..]);
        assert_eq!(messages.0.len(), 2);
        assert_eq!(messages.0[0].header.nlmsg_seq, 1);
        assert_eq!(messages.0[1].header.nlmsg_seq, 2);
    }

    #[test]
    fn test_netlink_request() {
        let mut req = Message::new(0, 0);

        let name = RouteAttr::new(libc::IFLA_IFNAME, "lo".as_bytes());
        req.add(&name.serialize().unwrap());

        let buf = req.serialize().unwrap();

        assert_eq!(buf.len(), 24);
        assert_eq!(req.header.nlmsg_len, 24);
    }
}
