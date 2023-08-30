use std::{io::Error, mem::size_of, os::fd::RawFd};

use anyhow::Result;

use crate::{consts, message::msg::NetlinkMessage};

pub struct NetlinkSocket {
    fd: RawFd,
    lsa: SockAddrNetlink,
}

impl NetlinkSocket {
    pub fn new(protocol: i32, pid: u32, groups: u32) -> Result<Self> {
        match unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_RAW | libc::SOCK_CLOEXEC,
                protocol,
            )
        } {
            fd if fd >= 0 => Ok(Self {
                fd,
                lsa: SockAddrNetlink::new(pid, groups),
            }),
            _ => Err(Error::last_os_error().into()),
        }
    }

    pub fn connect(protocol: i32, pid: u32, groups: u32) -> Result<Self> {
        let sock = Self::new(protocol, pid, groups)?;
        sock.bind()?;
        Ok(sock)
    }

    fn bind(&self) -> Result<()> {
        let (addr, len) = self.lsa.as_raw();

        match unsafe { libc::bind(self.fd, addr, len) } {
            res if res >= 0 => Ok(()),
            _ => Err(Error::last_os_error().into()),
        }
    }

    pub fn send(&self, buf: &[u8]) -> Result<()> {
        let (addr, len) = self.lsa.as_raw();

        match unsafe { libc::sendto(self.fd, buf.as_ptr() as _, buf.len(), 0, addr, len) } {
            res if res >= 0 => Ok(()),
            _ => Err(Error::last_os_error().into()),
        }
    }

    pub fn recv(&self) -> Result<(Vec<NetlinkMessage>, libc::sockaddr_nl)> {
        let mut from = unsafe { std::mem::zeroed::<libc::sockaddr_nl>() };
        let mut buf: [u8; consts::RECV_BUF_SIZE] = [0; consts::RECV_BUF_SIZE];
        match unsafe {
            libc::recvfrom(
                self.fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
                0,
                &mut from as *mut _ as *mut libc::sockaddr,
                &mut size_of::<libc::sockaddr_nl>().try_into().unwrap_or(0),
            )
        } {
            res if res >= 0 => {
                let msgs = NetlinkMessage::from(&buf[..res as usize])?;
                Ok((msgs, from))
            }
            _ => Err(Error::last_os_error().into()),
        }
    }

    pub fn pid(&self) -> Result<u32> {
        let mut rsa: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        match unsafe {
            libc::getsockname(
                self.fd,
                &mut rsa as *mut _ as *mut libc::sockaddr,
                &mut size_of::<libc::sockaddr_nl>().try_into().unwrap_or(0),
            )
        } {
            res if res >= 0 => Ok(rsa.nl_pid),
            _ => Err(Error::last_os_error().into()),
        }
    }
}

impl Drop for NetlinkSocket {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd) };
    }
}

// SockaddrNetlink implements the Sockaddr interface for AF_NETLINK type sockets.
#[derive(Default)]
pub struct SockAddrNetlink {
    pub family: u16,
    pub pad: u16,
    pub pid: u32,
    pub groups: u32,
}

impl SockAddrNetlink {
    pub fn new(pid: u32, groups: u32) -> Self {
        Self {
            family: libc::AF_NETLINK as u16,
            pid,
            groups,
            ..Default::default()
        }
    }

    pub fn as_raw(&self) -> (*const libc::sockaddr, libc::socklen_t) {
        (
            self as *const _ as *const libc::sockaddr,
            size_of::<SockAddrNetlink>() as libc::socklen_t,
        )
    }
}
