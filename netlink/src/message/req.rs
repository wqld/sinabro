use anyhow::Result;

use super::{nl::NetlinkHeader, rt::NetlinkPayload};

pub struct NetlinkRequest {
    pub header: NetlinkHeader,
    pub data: Option<Vec<u8>>,
}

impl NetlinkRequest {
    pub fn new(ptoro: u16, flags: i32) -> Self {
        Self {
            header: NetlinkHeader::new(ptoro, flags),
            data: None,
        }
    }

    pub fn serialize(&mut self) -> Result<Vec<u8>> {
        let mut buf = bincode::serialize(&self.header)?;
        if let Some(data) = &self.data {
            buf.extend_from_slice(data);
        }

        let len = buf.len() as u16;
        buf[..2].copy_from_slice(&len.to_ne_bytes());

        Ok(buf)
    }

    pub fn add_data<T: NetlinkPayload>(&mut self, data: &T) -> Result<()> {
        self.header.len += data.size() as u32;
        self.data
            .get_or_insert_with(|| Vec::with_capacity(data.size()))
            .extend_from_slice(&data.serialize()?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::message::rt::{Attribute, LinkHeaderBuilder};

    use super::*;

    #[test]
    fn test_serialize() {
        let mut req = NetlinkRequest::new(0, 0);

        let link_header = LinkHeaderBuilder::default()
            .family(0)
            .kind(772)
            .index(1)
            .flags(73)
            .build()
            .unwrap();

        req.add_data(&link_header).unwrap();

        // TODO NameAttribute
        let name_attr = Attribute::new(libc::IFLA_IFNAME, "lo".as_bytes().to_vec());

        // TODO varargs?
        req.add_data(&name_attr).unwrap();

        let buf = req.serialize().unwrap();
        assert_eq!(buf.len(), 40);
        assert_eq!(req.header.len, 38);
    }
}
