use std::ops::{Deref, DerefMut};

use anyhow::Result;

use crate::types::message::RouteAttrs;

#[derive(Default, Clone)]
pub struct GenlOp {
    pub id: u32,
    pub flags: u32,
}

impl TryFrom<&[u8]> for GenlOp {
    type Error = anyhow::Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        let attrs = RouteAttrs::from(bytes);
        let mut op = GenlOp::default();

        for attr in attrs {
            match attr.header.rta_type as i32 {
                libc::CTRL_ATTR_OP_ID => op.id = attr.payload.to_u32()?,
                libc::CTRL_ATTR_OP_FLAGS => op.flags = attr.payload.to_u32()?,
                _ => {}
            }
        }

        Ok(op)
    }
}

#[derive(Default, Clone)]
pub struct GenlOps(Vec<GenlOp>);

impl TryFrom<&[u8]> for GenlOps {
    type Error = anyhow::Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        let attrs = RouteAttrs::from(bytes);
        let ops: Result<Vec<_>> = attrs
            .iter()
            .map(|attr| GenlOp::try_from(attr.payload.as_slice()))
            .collect();

        Ok(Self(ops?))
    }
}

impl Deref for GenlOps {
    type Target = Vec<GenlOp>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Default, Clone)]
pub struct GenlMulticastGroup {
    pub id: u32,
    pub name: String,
}

impl TryFrom<&[u8]> for GenlMulticastGroup {
    type Error = anyhow::Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        let attrs = RouteAttrs::from(bytes);
        let mut group = GenlMulticastGroup::default();

        for attr in attrs {
            match attr.header.rta_type as i32 {
                libc::CTRL_ATTR_MCAST_GRP_ID => group.id = attr.payload.to_u32()?,
                libc::CTRL_ATTR_MCAST_GRP_NAME => group.name = attr.payload.to_string()?,
                _ => {}
            }
        }

        Ok(group)
    }
}

#[derive(Default, Clone)]
pub struct GenlMulticastGroups(Vec<GenlMulticastGroup>);

impl TryFrom<&[u8]> for GenlMulticastGroups {
    type Error = anyhow::Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        let attrs = RouteAttrs::from(bytes);
        let groups: Result<Vec<_>> = attrs
            .iter()
            .map(|attr| GenlMulticastGroup::try_from(attr.payload.as_slice()))
            .collect();

        Ok(Self(groups?))
    }
}

impl Deref for GenlMulticastGroups {
    type Target = Vec<GenlMulticastGroup>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Default, Clone)]
pub struct GenlFamily {
    pub id: u16,
    pub header_size: u32,
    pub name: String,
    pub version: u32,
    pub max_attr: u32,
    pub ops: GenlOps,
    pub groups: GenlMulticastGroups,
}

impl TryFrom<&RouteAttrs> for GenlFamily {
    type Error = anyhow::Error;

    fn try_from(attrs: &RouteAttrs) -> Result<Self> {
        let mut family = GenlFamily::default();

        for attr in attrs.iter() {
            let payload_slice = attr.payload.as_slice();
            match attr.header.rta_type as i32 {
                libc::CTRL_ATTR_FAMILY_ID => family.id = attr.payload.to_u16()?,
                libc::CTRL_ATTR_FAMILY_NAME => family.name = attr.payload.to_string()?,
                libc::CTRL_ATTR_VERSION => family.version = attr.payload.to_u32()?,
                libc::CTRL_ATTR_HDRSIZE => family.header_size = attr.payload.to_u32()?,
                libc::CTRL_ATTR_MAXATTR => family.max_attr = attr.payload.to_u32()?,
                libc::CTRL_ATTR_OPS => family.ops = GenlOps::try_from(payload_slice)?,
                libc::CTRL_ATTR_MCAST_GROUPS => {
                    family.groups = GenlMulticastGroups::try_from(payload_slice)?
                }
                _ => {}
            }
        }

        Ok(family)
    }
}

pub struct GenlFamilies(Vec<GenlFamily>);

impl Deref for GenlFamilies {
    type Target = Vec<GenlFamily>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for GenlFamilies {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TryFrom<Vec<Vec<u8>>> for GenlFamilies {
    type Error = anyhow::Error;

    fn try_from(msgs: Vec<Vec<u8>>) -> Result<Self> {
        let families: Result<Vec<_>> = msgs
            .iter()
            .map(|msg| {
                let attrs = RouteAttrs::from(&msg.as_slice()[4..]);
                GenlFamily::try_from(&attrs)
            })
            .collect();

        Ok(Self(families?))
    }
}
