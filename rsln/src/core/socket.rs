use std::{
    io::{Error, Result},
    mem::{size_of, zeroed},
    os::fd::RawFd,
};

use libc::{c_void, size_t, sockaddr, sockaddr_nl, socklen_t, AF_NETLINK, SOCK_CLOEXEC, SOCK_RAW};

use super::message::Messages;

const RECV_BUF_SIZE: usize = 65536;

#[derive(Clone)]
pub struct Socket {
    fd: RawFd,
    sa: SocketAddr,
}

impl Socket {
    pub fn new(proto: i32, pid: u32, groups: u32) -> Result<Self> {
        match unsafe { libc::socket(AF_NETLINK, SOCK_RAW | SOCK_CLOEXEC, proto) } {
            -1 => Err(Error::last_os_error()),
            fd => {
                let sa = SocketAddr::new(pid, groups);
                let s = Self { fd, sa };
                s.bind()?;
                Ok(s)
            }
        }
    }

    fn bind(&self) -> Result<()> {
        let (addr, addr_len) = self.sa.as_raw();

        match unsafe { libc::bind(self.fd, addr, addr_len) } {
            -1 => Err(Error::last_os_error()),
            _ => Ok(()),
        }
    }

    pub fn block(&self) -> Result<()> {
        match unsafe {
            libc::fcntl(
                self.fd,
                libc::F_SETFL,
                libc::fcntl(self.fd, libc::F_GETFL, 0) & !libc::O_NONBLOCK,
            )
        } {
            -1 => Err(Error::last_os_error()),
            _ => Ok(()),
        }
    }

    pub fn non_block(&self) -> Result<()> {
        match unsafe {
            libc::fcntl(
                self.fd,
                libc::F_SETFL,
                libc::fcntl(self.fd, libc::F_GETFL, 0) | libc::O_NONBLOCK,
            )
        } {
            -1 => Err(Error::last_os_error()),
            _ => Ok(()),
        }
    }

    pub fn send(&self, buf: &[u8]) -> Result<()> {
        let (addr, addr_len) = self.sa.as_raw();

        match unsafe {
            libc::sendto(
                self.fd,
                buf.as_ptr() as *const c_void,
                buf.len() as size_t,
                0,
                addr,
                addr_len,
            )
        } {
            -1 => Err(Error::last_os_error()),
            _ => Ok(()),
        }
    }

    pub fn recv(&self) -> Result<(Messages, sockaddr_nl)> {
        let mut from: sockaddr_nl = unsafe { zeroed() };
        let mut buf: [u8; RECV_BUF_SIZE] = [0; RECV_BUF_SIZE];

        match unsafe {
            libc::recvfrom(
                self.fd,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as size_t,
                0,
                &mut from as *mut _ as *mut sockaddr,
                &mut size_of::<sockaddr_nl>() as *mut _ as *mut socklen_t,
            )
        } {
            -1 => Err(Error::last_os_error()),
            ret => Ok((Messages::from(&buf[..ret as usize]), from)),
        }
    }

    pub fn pid(&self) -> Result<u32> {
        let mut rsa: sockaddr_nl = unsafe { zeroed() };

        match unsafe {
            libc::getsockname(
                self.fd,
                &mut rsa as *mut _ as *mut sockaddr,
                &mut size_of::<sockaddr_nl>() as *mut _ as *mut socklen_t,
            )
        } {
            -1 => Err(Error::last_os_error()),
            _ => Ok(rsa.nl_pid),
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

#[derive(Clone)]
struct SocketAddr(sockaddr_nl);

impl SocketAddr {
    fn new(pid: u32, groups: u32) -> Self {
        let mut addr: sockaddr_nl = unsafe { zeroed() };
        addr.nl_family = AF_NETLINK as u16;
        addr.nl_pid = pid;
        addr.nl_groups = groups;
        Self(addr)
    }

    fn as_raw(&self) -> (*const sockaddr, socklen_t) {
        (
            &self.0 as *const _ as *const sockaddr,
            size_of::<sockaddr_nl>() as socklen_t,
        )
    }
}

#[cfg(test)]
mod tests {
    use libc::NETLINK_ROUTE;

    use super::*;

    #[test]
    fn test_netlink_socket() {
        let s = Socket::new(NETLINK_ROUTE, 0, 0).unwrap();

        assert!(s.pid().unwrap() > 0);

        let sa = s.sa.as_raw();
        let sa: sockaddr_nl = unsafe { *(sa.0 as *const sockaddr_nl) };

        assert_eq!(sa.nl_family, AF_NETLINK as u16);
        assert_eq!(sa.nl_pid, 0);
        assert_eq!(sa.nl_groups, 0);

        // This is a valid message for listing the network links on the system
        let msg = [
            0x14, 0x00, 0x00, 0x00, 0x12, 0x00, 0x01, 0x03, 0xfd, 0xfe, 0x38, 0x5c, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        assert!(s.send(&msg[..]).is_ok());

        let (netlink_msgs, from) = s.recv().unwrap();

        assert_eq!(from.nl_pid, 0);
        assert_eq!(from.nl_groups, 0);

        assert!(netlink_msgs.len() > 0);
    }

    #[test]
    fn test_socket_addr() {
        let sa = SocketAddr::new(1, 2);
        assert_eq!(sa.0.nl_family, AF_NETLINK as u16);
        assert_eq!(sa.0.nl_pid, 1);
        assert_eq!(sa.0.nl_groups, 2);

        let (addr, addr_len) = sa.as_raw();
        let addr: sockaddr_nl = unsafe { *(addr as *const sockaddr_nl) };

        assert_eq!(addr.nl_family, AF_NETLINK as u16);
        assert_eq!(addr.nl_pid, 1);
        assert_eq!(addr.nl_groups, 2);
        assert_eq!(addr_len, size_of::<sockaddr_nl>() as socklen_t);
    }
}
