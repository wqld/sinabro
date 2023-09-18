use std::{
    io::Error,
    mem::{size_of, zeroed},
    os::fd::RawFd,
};

use anyhow::Result;

use crate::{consts, message::nl::NetlinkMessages};

pub struct NetlinkSocket {
    fd: RawFd,
}

impl NetlinkSocket {
    fn new(proto: i32) -> Result<Self> {
        match unsafe { libc::socket(libc::AF_NETLINK, libc::SOCK_RAW | libc::SOCK_CLOEXEC, proto) }
        {
            -1 => Err(Error::last_os_error().into()),
            fd => Ok(Self { fd }),
        }
    }

    pub fn connect(proto: i32, pid: u32, groups: u32) -> Result<Self> {
        let sock = Self::new(proto)?;
        sock.bind(pid, groups)?;
        Ok(sock)
    }

    fn bind(&self, pid: u32, groups: u32) -> Result<()> {
        let mut addr = unsafe { zeroed::<libc::sockaddr_nl>() };
        addr.nl_family = libc::AF_NETLINK as u16;
        addr.nl_pid = pid;
        addr.nl_groups = groups;

        match unsafe {
            libc::bind(
                self.fd,
                &addr as *const _ as *const libc::sockaddr,
                size_of::<libc::sockaddr_nl>() as u32,
            )
        } {
            -1 => Err(Error::last_os_error().into()),
            _ => Ok(()),
        }
    }

    pub fn send(&self, buf: &[u8]) -> Result<()> {
        match unsafe { libc::send(self.fd, buf.as_ptr() as _, buf.len(), 0) } {
            -1 => Err(Error::last_os_error().into()),
            _ => Ok(()),
        }
    }

    pub fn recv(&self) -> Result<(NetlinkMessages, libc::sockaddr_nl)> {
        let mut from = unsafe { zeroed::<libc::sockaddr_nl>() };
        let mut buf = [0; consts::RECV_BUF_SIZE];
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
            -1 => Err(Error::last_os_error().into()),
            len => Ok((buf[..len as usize].into(), from)),
        }
    }

    pub fn pid(&self) -> Result<u32> {
        let mut rsa = unsafe { zeroed::<libc::sockaddr_nl>() };
        match unsafe {
            libc::getsockname(
                self.fd,
                &mut rsa as *mut _ as *mut libc::sockaddr,
                &mut size_of::<libc::sockaddr_nl>().try_into().unwrap_or(0),
            )
        } {
            -1 => Err(Error::last_os_error().into()),
            _ => Ok(rsa.nl_pid),
        }
    }
}

impl Drop for NetlinkSocket {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd) };
    }
}

// SockaddrNetlink implements the Sockaddr interface for AF_NETLINK type sockets.
pub struct SockAddrNetlink(libc::sockaddr_nl);

impl SockAddrNetlink {
    pub fn new(pid: u32, groups: u32) -> Self {
        let mut addr = unsafe { std::mem::zeroed::<libc::sockaddr_nl>() };
        addr.nl_family = libc::AF_NETLINK as u16;
        addr.nl_pid = pid;
        addr.nl_groups = groups;
        Self(addr)
    }

    pub fn as_raw(&self) -> (*const libc::sockaddr, u32) {
        (
            self as *const _ as *const libc::sockaddr,
            size_of::<SockAddrNetlink>() as u32,
        )
    }
}

#[cfg(test)]
mod tests {

    use crate::{message::rt::LinkHeader, utils::deserialize};

    use super::*;

    #[test]
    fn test_netlink_socket() {
        let s = NetlinkSocket::connect(libc::NETLINK_ROUTE, 0, 0).unwrap();

        // This is a valid message for listing the network links on the system
        let msg = [
            0x14, 0x00, 0x00, 0x00, 0x12, 0x00, 0x01, 0x03, 0xfd, 0xfe, 0x38, 0x5c, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        s.send(&msg[..]).unwrap();

        let pid = s.pid().unwrap();
        let mut res: Vec<Vec<u8>> = Vec::new();

        'done: loop {
            let (netlink_msgs, from) = s.recv().unwrap();

            if from.nl_pid != consts::PID_KERNEL {
                println!("received message from unknown source");
                continue;
            }

            for m in netlink_msgs {
                if m.header.pid != pid {
                    println!("received message with wrong pid");
                    continue;
                }

                match m.header.kind {
                    consts::NLMSG_ERROR => {
                        println!("the kernel responded with an error");
                        return;
                    }
                    consts::NLMSG_DONE => {
                        break 'done;
                    }
                    _ => {
                        res.push(m.payload);
                    }
                }
            }
        }

        res.iter().for_each(|r| {
            let msg = deserialize::<LinkHeader>(r);
            println!("{:?}", msg);
        });
    }
}
