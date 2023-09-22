use derive_builder::Builder;

use crate::{consts, message::rt::LinkHeader};

pub enum Link {
    Device,
    Dummy,
    Bridge,
    Veth,
}

#[derive(Builder)]
pub struct LinkAttributes {
    #[builder(default)]
    pub name: String,
    #[builder(default)]
    pub kind: String,
    #[builder(default)]
    pub index: i32,
    #[builder(default)]
    pub hw_addr: Vec<u8>,
    #[builder(default)]
    pub mtu: u32,
    #[builder(default)]
    pub flags: u32,
    #[builder(default)]
    pub raw_flags: u32,
    #[builder(default)]
    pub parent_index: i32,
    #[builder(default)]
    pub master_index: i32,
    #[builder(default)]
    pub tx_queue_len: i32,
    #[builder(default)]
    pub alias: String,
    #[builder(default)]
    pub promisc: i32,
    #[builder(default)]
    pub all_multi: i32,
    #[builder(default)]
    pub multicast: i32,
    // pub xdp: LinkXdp, TODO
    #[builder(default)]
    pub encap_type: String,
    #[builder(default)]
    pub prot_info: String,
    #[builder(default)]
    pub oper_state: u8,
    #[builder(default)]
    pub phys_switch_id: i32,
    #[builder(default)]
    pub netns_id: i32,
    #[builder(default)]
    pub gso_max_size: u32,
    #[builder(default)]
    pub gso_max_segs: u32,
    #[builder(default)]
    pub gro_max_size: u32,
    #[builder(default)]
    pub vfs: String,
    #[builder(default)]
    pub num_tx_queues: i32,
    #[builder(default)]
    pub num_rx_queues: i32,
    #[builder(default)]
    pub group: u32,
    #[builder(default)]
    pub statistics: String,
}

impl<'a> From<&'a LinkHeader> for LinkAttributes {
    fn from(header: &'a LinkHeader) -> Self {
        LinkAttributesBuilder::default()
            .index(header.index)
            .raw_flags(header.flags)
            .flags({
                let mut flags = 0;

                if header.flags & consts::IFF_UP != 0 {
                    flags |= 1 // consts::IFF_UP;
                }

                if header.flags & consts::IFF_BROADCAST != 0 {
                    flags |= 2 // consts::IFF_BROADCAST;
                }

                if header.flags & consts::IFF_LOOPBACK != 0 {
                    flags |= 4 // consts::IFF_LOOPBACK;
                }

                if header.flags & consts::IFF_POINTOPOINT != 0 {
                    flags |= 8 // consts::IFF_POINTOPOINT;
                }

                if header.flags & consts::IFF_MULTICAST != 0 {
                    flags |= 16 // consts::IFF_MULTICAST;
                }

                flags
            })
            .promisc((header.flags & consts::IFF_PROMISC) as i32)
            .all_multi((header.flags & consts::IFF_ALLMULTI) as i32)
            .multicast((header.flags & consts::IFF_MULTICAST) as i32)
            .encap_type(match header.kind {
                0 => "generic".to_string(),
                1 => "ether".to_string(),
                _ => "unknown".to_string(),
            })
            .build()
            .unwrap()
    }
}
