#![no_std]

pub const CLUSTER_CIDR_KEY: u8 = 0;
pub const HOST_IP_KEY: u8 = 1;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct NatKey {
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for NatKey {}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct OriginValue {
    pub ip: u32,
    pub dummy: u16,
    pub port: u16,
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for OriginValue {}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct NetworkInfo {
    pub ip: u32,
    pub subnet_mask: u32,
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for NetworkInfo {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SockKey {
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u32,
    pub dst_port: u32,
    pub family: u32,
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for SockKey {}
