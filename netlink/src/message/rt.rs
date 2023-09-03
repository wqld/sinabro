use std::{mem::size_of, slice::from_raw_parts};

#[repr(C)]
#[derive(Debug)]
pub struct IfInfoMessage {
    pub family: u8,
    pub _pad: u8,
    pub kind: u16,
    pub index: i32,
    pub flags: u32,
    pub change: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct AddressMessage {
    pub family: u8,
    pub prefix_len: u8,
    pub flags: u8,
    pub scope: u8,
    pub index: i32,
}

#[repr(C)]
#[derive(Debug)]
pub struct RouteMessage {
    pub family: u8,
    pub dst_len: u8,
    pub src_len: u8,
    pub tos: u8,
    pub table: u8,
    pub protocol: u8,
    pub scope: u8,
    pub kind: u8,
    pub flags: u32,
}

pub trait NetlinkRequest {}

impl NetlinkRequest for IfInfoMessage {}

impl NetlinkRequest for AddressMessage {}

impl NetlinkRequest for RouteMessage {}

pub fn serialize<T: NetlinkRequest>(req: &T) -> &[u8] {
    unsafe { from_raw_parts((req as *const T) as *const u8, size_of::<T>()) }
}

pub fn deserialize<T: NetlinkRequest>(buf: &[u8]) -> &T {
    unsafe { &*(buf[..size_of::<T>()].as_ptr() as *const T) }
}
