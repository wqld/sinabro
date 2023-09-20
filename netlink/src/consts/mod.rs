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

pub enum AttributeKind {
    Name = 3,
}
