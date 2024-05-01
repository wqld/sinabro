#![no_std]
#![no_main]

use core::mem;

use aya_ebpf::bindings::sk_action::SK_PASS;
use aya_ebpf::bindings::{
    sk_msg_md, BPF_ANY, BPF_F_INGRESS, BPF_F_PSEUDO_HDR, BPF_SOCK_OPS_ACTIVE_ESTABLISHED_CB,
    BPF_SOCK_OPS_PASSIVE_ESTABLISHED_CB, BPF_SOCK_OPS_STATE_CB_FLAG, TC_ACT_PIPE, TC_ACT_SHOT,
};
use aya_ebpf::maps::SockHash;
use aya_ebpf::{
    cty::c_long,
    helpers::{bpf_csum_diff, bpf_get_prandom_u32},
    macros::{classifier, map, sk_msg, sock_ops},
    maps::HashMap,
    programs::{SkMsgContext, SockOpsContext, TcContext},
};
use aya_log_ebpf::{error, info};
use common::{NatKey, NetworkInfo, OriginValue, SockKey, CLUSTER_CIDR_KEY, HOST_IP_KEY};
use memoffset::offset_of;
use network_types::{
    eth::{EthHdr, EtherType},
    ip::{IpProto, Ipv4Hdr},
    tcp::TcpHdr,
};

#[map]
pub static mut SOCK_OPS_MAP: SockHash<SockKey> = SockHash::with_max_entries(65535, 0);

#[map]
static mut NET_CONFIG_MAP: HashMap<u8, NetworkInfo> = HashMap::with_max_entries(2, 0);

#[map]
static mut NODE_MAP: HashMap<u32, u8> = HashMap::with_max_entries(128, 0);

#[map]
static mut SNAT_IPV4_MAP: HashMap<NatKey, OriginValue> = HashMap::with_max_entries(128, 0);

#[classifier]
pub fn tc_ingress(ctx: TcContext) -> i32 {
    match try_tc_ingress(ctx) {
        Ok(ret) => ret,
        Err(_) => TC_ACT_SHOT,
    }
}

fn try_tc_ingress(ctx: TcContext) -> Result<i32, ()> {
    let eth_hdr: EthHdr = ctx.load(0).map_err(|_| ())?;
    match eth_hdr.ether_type {
        EtherType::Ipv4 => {
            let ipv4hdr: Ipv4Hdr = ctx.load(EthHdr::LEN).map_err(|_| ())?;
            match ipv4hdr.proto {
                IpProto::Tcp => handle_tcp_ingress(ctx),
                _ => Ok(TC_ACT_PIPE),
            }
        }
        _ => Ok(TC_ACT_PIPE),
    }
}

fn handle_tcp_ingress(mut ctx: TcContext) -> Result<i32, ()> {
    let ip_hdr: Ipv4Hdr = ctx.load(EthHdr::LEN).map_err(|_| ())?;
    let tcp_hdr: TcpHdr = ctx.load(EthHdr::LEN + Ipv4Hdr::LEN).map_err(|_| ())?;

    let src_ip = u32::from_be(ip_hdr.src_addr);
    let src_port = u16::from_be(tcp_hdr.source);

    let dst_ip = u32::from_be(ip_hdr.dst_addr);
    let dst_port = u16::from_be(tcp_hdr.dest);

    let cluster_cidr = unsafe { NET_CONFIG_MAP.get(&CLUSTER_CIDR_KEY).ok_or(()) }?;

    if is_ip_in_cidr(src_ip, cluster_cidr) {
        return Ok(TC_ACT_PIPE);
    }

    let nat_key = NatKey {
        src_ip: dst_ip,
        dst_ip: src_ip,
        src_port: dst_port,
        dst_port: src_port,
    };

    let origin_value = unsafe {
        match SNAT_IPV4_MAP.get(&nat_key) {
            Some(value) => value,
            None => {
                return Ok(TC_ACT_PIPE);
            }
        }
    };

    if origin_value.ip == dst_ip && origin_value.port == dst_port {
        info!(&ctx, "no need to dnat");
        return Ok(TC_ACT_PIPE);
    }

    snat_v4_rewrite_headers(
        &mut ctx,
        ip_hdr.dst_addr,
        origin_value.ip.to_be(),
        offset_of!(Ipv4Hdr, dst_addr),
        tcp_hdr.dest,
        origin_value.port.to_be(),
        offset_of!(TcpHdr, dest),
    )
    .map_err(|_| ())?;

    info!(
        &ctx,
        "ingress: {:i}:{} -> {:i}:{} / dnat: {:i}:{}",
        src_ip,
        src_port,
        dst_ip,
        dst_port,
        origin_value.ip,
        origin_value.port
    );

    Ok(TC_ACT_PIPE)
}

#[classifier]
pub fn tc_egress(ctx: TcContext) -> i32 {
    match try_tc_egress(ctx) {
        Ok(ret) => ret,
        Err(_) => TC_ACT_SHOT,
    }
}

fn try_tc_egress(ctx: TcContext) -> Result<i32, ()> {
    let eth_hdr: EthHdr = ctx.load(0).map_err(|_| ())?;
    match eth_hdr.ether_type {
        EtherType::Ipv4 => {
            let ipv4hdr: Ipv4Hdr = ctx.load(EthHdr::LEN).map_err(|_| ())?;
            match ipv4hdr.proto {
                IpProto::Tcp => handle_tcp_egress(ctx),
                _ => Ok(TC_ACT_PIPE),
            }
        }
        _ => Ok(TC_ACT_PIPE),
    }
}

fn handle_tcp_egress(mut ctx: TcContext) -> Result<i32, ()> {
    let ip_hdr: Ipv4Hdr = ctx.load(EthHdr::LEN).map_err(|_| ())?;
    let tcp_hdr: TcpHdr = ctx.load(EthHdr::LEN + Ipv4Hdr::LEN).map_err(|_| ())?;

    let dst_ip = u32::from_be(ip_hdr.dst_addr);
    let dst_port = u16::from_be(tcp_hdr.dest);

    let cluster_cidr = unsafe { NET_CONFIG_MAP.get(&CLUSTER_CIDR_KEY).ok_or(()) }?;

    if is_ip_in_cidr(dst_ip, cluster_cidr) {
        return Ok(TC_ACT_PIPE);
    }

    let src_ip = u32::from_be(ip_hdr.src_addr);
    let src_port = u16::from_be(tcp_hdr.source);

    if is_node_ip(src_ip) {
        return Ok(TC_ACT_PIPE);
    }

    let nat_ip = unsafe { NET_CONFIG_MAP.get(&HOST_IP_KEY).ok_or(()) }?.ip;
    let nat_port = snat_try_keep_port(30000_u16, 60000_u16, src_port);

    // TODO: use conntrack to track tcp connection

    snat_v4_rewrite_headers(
        &mut ctx,
        ip_hdr.src_addr,
        nat_ip.to_be(),
        offset_of!(Ipv4Hdr, src_addr),
        tcp_hdr.source,
        nat_port.to_be(),
        offset_of!(TcpHdr, source),
    )
    .map_err(|_| ())?;

    let nat_key = NatKey {
        src_ip: nat_ip,
        dst_ip,
        src_port: nat_port,
        dst_port,
    };

    let origin_value = OriginValue {
        ip: src_ip,
        dummy: 0,
        port: src_port,
    };

    unsafe {
        SNAT_IPV4_MAP
            .insert(&nat_key, &origin_value, 0)
            .map_err(|_| ())
    }?;

    info!(
        &ctx,
        "egress: {:i}:{} -> {:i}:{} / snat: {:i}:{}",
        src_ip,
        src_port,
        dst_ip,
        dst_port,
        nat_ip,
        nat_port
    );

    Ok(TC_ACT_PIPE)
}

#[inline(always)]
fn snat_v4_rewrite_headers(
    ctx: &mut TcContext,
    old_addr: u32,
    new_addr: u32,
    addr_offset: usize,
    old_port: u16,
    new_port: u16,
    port_offset: usize,
) -> Result<(), c_long> {
    let sum = unsafe {
        bpf_csum_diff(
            &old_addr as *const _ as *mut _,
            4,
            &new_addr as *const _ as *mut _,
            4,
            0,
        )
    } as u64;

    ctx.store(EthHdr::LEN + addr_offset, &new_addr, 0)?;

    ctx.l4_csum_replace(
        EthHdr::LEN + Ipv4Hdr::LEN + offset_of!(TcpHdr, check),
        old_port as u64,
        new_port as u64,
        mem::size_of_val(&new_port) as u64,
    )?;

    ctx.store(EthHdr::LEN + Ipv4Hdr::LEN + port_offset, &new_port, 0)?;

    ctx.l4_csum_replace(
        EthHdr::LEN + Ipv4Hdr::LEN + offset_of!(TcpHdr, check),
        0,
        sum,
        BPF_F_PSEUDO_HDR as u64,
    )?;

    ctx.l3_csum_replace(EthHdr::LEN + offset_of!(Ipv4Hdr, check), 0, sum, 0)?;

    Ok(())
}

#[inline(always)]
fn snat_clamp_port_range(start: u16, end: u16, val: u16) -> u16 {
    (val % (end - start)) + start
}

#[inline(always)]
fn snat_try_keep_port(start: u16, end: u16, val: u16) -> u16 {
    if val >= start && val <= end {
        val
    } else {
        snat_clamp_port_range(start, end, unsafe { bpf_get_prandom_u32() } as u16)
    }
}

fn is_ip_in_cidr(ip: u32, cidr: &NetworkInfo) -> bool {
    if is_node_ip(ip) {
        return true;
    }

    let network_addr = cidr.ip & cidr.subnet_mask;
    let masked_ip = ip & cidr.subnet_mask;
    network_addr == masked_ip
}

fn is_node_ip(ip: u32) -> bool {
    unsafe { NODE_MAP.get(&ip).is_some() }
}

#[sock_ops]
pub fn tcp_accelerate(ctx: SockOpsContext) -> u32 {
    try_tcp_accelerate(ctx).unwrap_or(0)
}

fn try_tcp_accelerate(ctx: SockOpsContext) -> Result<u32, ()> {
    let family = ctx.family();

    // currently only support IPv4
    if family != 2 {
        return Ok(0);
    }

    match ctx.op() {
        BPF_SOCK_OPS_ACTIVE_ESTABLISHED_CB | BPF_SOCK_OPS_PASSIVE_ESTABLISHED_CB => {
            // info!(
            //     &ctx,
            //     "<<< ipv4 op = {}, src {:i}:{} => dst {:i}:{}",
            //     ctx.op(),
            //     u32::from_be(ctx.local_ip4()),
            //     ctx.local_port(),
            //     u32::from_be(ctx.remote_ip4()),
            //     u32::from_be(ctx.remote_port())
            // );

            let mut sock_key = extract_sock_key_from(&ctx);

            unsafe {
                SOCK_OPS_MAP
                    .update(&mut sock_key, &mut *ctx.ops, BPF_ANY.into())
                    .map_err(|e| {
                        error!(&ctx, "failed to update SOCK_OPS_MAP: {}", e);
                    })?;
            }

            ctx.set_cb_flags(BPF_SOCK_OPS_STATE_CB_FLAG as i32)
                .map_err(|e| {
                    error!(&ctx, "failed to set BPF_SOCK_OPS_STATE_CB_FLAG: {}", e);
                })?;
        }
        // BPF_SOCK_OPS_STATE_CB => match ctx.arg(1) {
        //     BPF_TCP_CLOSE | BPF_TCP_CLOSE_WAIT | BPF_TCP_LAST_ACK => {
        //         // info!(
        //         //     &ctx,
        //         //     ">>> ipv4 op = {}, src {:i}:{} => dst {:i}:{}, state: {}",
        //         //     ctx.op(),
        //         //     u32::from_be(ctx.local_ip4()),
        //         //     ctx.local_port(),
        //         //     u32::from_be(ctx.remote_ip4()),
        //         //     u32::from_be(ctx.remote_port()),
        //         //     ctx.arg(1)
        //         // );
        //     }
        //     _ => {}
        // },
        _ => {}
    }

    Ok(0)
}

fn extract_sock_key_from(ctx: &SockOpsContext) -> SockKey {
    SockKey {
        src_ip: u32::from_be(ctx.local_ip4()),
        dst_ip: u32::from_be(ctx.remote_ip4()),
        src_port: ctx.local_port(),
        dst_port: u32::from_be(ctx.remote_port()),
        family: ctx.family(),
    }
}

#[sk_msg]
pub fn tcp_bypass(ctx: SkMsgContext) -> u32 {
    try_tcp_bypass(ctx).unwrap_or(SK_PASS)
}

fn try_tcp_bypass(ctx: SkMsgContext) -> Result<u32, ()> {
    // info!(&ctx, "received a message on the socket");

    let msg = unsafe { &*ctx.msg };

    if msg.family != 2 {
        return Ok(SK_PASS);
    }

    let mut sock_key = sk_msg_extract_key(msg);

    unsafe { SOCK_OPS_MAP.redirect_msg(&ctx, &mut sock_key, BPF_F_INGRESS as u64) };
    // info!(
    //     &ctx,
    //     "tcp_bypass: {:i}:{} <-> {:i}:{} / ret: {}",
    //     sock_key.src_ip,
    //     sock_key.src_port,
    //     sock_key.dst_ip,
    //     sock_key.dst_port,
    //     ret
    // );

    Ok(SK_PASS)
}

fn sk_msg_extract_key(msg: &sk_msg_md) -> SockKey {
    SockKey {
        src_ip: u32::from_be(msg.remote_ip4),
        dst_ip: u32::from_be(msg.local_ip4),
        src_port: u32::from_be(msg.remote_port),
        dst_port: msg.local_port,
        family: msg.family,
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
