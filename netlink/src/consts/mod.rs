pub const NLMSG_HDR_LEN: usize = 0x10;
pub const NLMSG_ALIGN_TO: usize = 0x4;

pub const NLMSG_ERROR: u16 = 2;
pub const NLMSG_DONE: u16 = 3;

pub const RECV_BUF_SIZE: usize = 65536;

pub const PID_KERNEL: u32 = 0;

pub const RT_ATTR_SIZE: usize = 0x4;
pub const IF_INFO_MSG_SIZE: usize = 0x10;
pub const ADDR_MSG_SIZE: usize = 0x8;
pub const ROUTE_MSG_SIZE: usize = 0xC;

pub const RTA_ALIGNTO: usize = 0x4;

pub const IFF_UP: u32 = 0x1;
pub const IFF_BROADCAST: u32 = 0x2;
pub const IFF_DEBUG: u32 = 0x4;
pub const IFF_LOOPBACK: u32 = 0x8;
pub const IFF_POINTOPOINT: u32 = 0x10;
pub const IFF_NOTRAILERS: u32 = 0x20;
pub const IFF_RUNNING: u32 = 0x40;
pub const IFF_NOARP: u32 = 0x80;
pub const IFF_PROMISC: u32 = 0x100;
pub const IFF_ALLMULTI: u32 = 0x200;
pub const IFF_MASTER: u32 = 0x400;
pub const IFF_SLAVE: u32 = 0x800;
pub const IFF_MULTICAST: u32 = 0x1000;
pub const IFF_PORTSEL: u32 = 0x2000;
pub const IFF_AUTOMEDIA: u32 = 0x4000;
pub const IFF_DYNAMIC: u32 = 0x8000;

pub enum AttributeKind {
    Name = 3,
}
