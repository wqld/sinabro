use std::ops::{Deref, DerefMut};

use anyhow::{anyhow, Result};

use crate::{
    core::message::Message,
    handle::zero_terminated,
    types::{
        generic::{GenlFamilies, GenlFamily},
        message::{Attribute, GenlMessage, RouteAttr},
    },
};

use super::sock_handle::SocketHandle;

pub struct GenericHandle<'a> {
    pub socket: &'a mut SocketHandle,
}

impl<'a> Deref for GenericHandle<'a> {
    type Target = SocketHandle;

    fn deref(&self) -> &Self::Target {
        self.socket
    }
}

impl DerefMut for GenericHandle<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.socket
    }
}

impl<'a> From<&'a mut SocketHandle> for GenericHandle<'a> {
    fn from(socket: &'a mut SocketHandle) -> Self {
        Self { socket }
    }
}

impl GenericHandle<'_> {
    pub fn list_family(&mut self) -> Result<GenlFamilies> {
        let mut req = Message::new(libc::GENL_ID_CTRL as u16, libc::NLM_F_DUMP);
        let msg = GenlMessage::get_family_message();

        req.add(&msg.serialize()?);

        let msgs = self.request(&mut req, 0)?;

        GenlFamilies::try_from(msgs)
    }

    pub fn get_family(&mut self, name: &str) -> Result<GenlFamily> {
        let mut req = Message::new(libc::GENL_ID_CTRL as u16, 0);
        let msg = GenlMessage::get_family_message();
        let family_name =
            RouteAttr::new(libc::CTRL_ATTR_FAMILY_NAME as u16, &zero_terminated(name));

        req.add(&msg.serialize()?);
        req.add(&family_name.serialize()?);

        let msgs = self.request(&mut req, 0)?;

        GenlFamilies::try_from(msgs)?
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("invalid response for GENL_CTRL_CMD_GETFAMILY"))
    }
}
