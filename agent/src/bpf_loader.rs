use std::net::Ipv4Addr;
use std::sync::Arc;

use aya::maps::HashMap;
use aya::programs::{tc, SchedClassifier, TcAttachType};
use aya::{include_bytes_aligned, Bpf};
use aya_log::BpfLogger;
use common::{NetworkInfo, CLUSTER_CIDR_KEY, HOST_IP_KEY};
use tokio::sync::Notify;
use tracing::warn;

use crate::server::api_server;

pub struct BpfLoader {
    iface: String,
}

impl BpfLoader {
    pub fn new(iface: &str) -> Self {
        Self {
            iface: iface.to_string(),
        }
    }

    pub async fn load(
        &self,
        host_ip: &str,
        cluster_cidr: &str,
        pod_cidr: &str,
        store_path: &str,
        shutdown: Arc<Notify>,
    ) -> anyhow::Result<()> {
        // This will include your eBPF object file as raw bytes at compile-time and load it at
        // runtime. This approach is recommended for most real-world use cases. If you would
        // like to specify the eBPF program at runtime rather than at compile-time, you can
        // reach for `Bpf::load_file` instead.
        #[cfg(debug_assertions)]
        let mut bpf = Bpf::load(include_bytes_aligned!(
            "../../target/bpfel-unknown-none/debug/ebpf"
        ))?;
        #[cfg(not(debug_assertions))]
        let mut bpf = Bpf::load(include_bytes_aligned!(
            "../../target/bpfel-unknown-none/release/ebpf"
        ))?;
        if let Err(e) = BpfLogger::init(&mut bpf) {
            // This can happen if you remove all log statements from your eBPF program.
            warn!("failed to initialize eBPF logger: {}", e);
        }
        // error adding clsact to the interface if it is already added is harmless
        // the full cleanup can be done with 'sudo tc qdisc del dev eth0 clsact'.
        let _ = tc::qdisc_add_clsact(&self.iface);

        let tc_ingress: &mut SchedClassifier = bpf.program_mut("tc_ingress").unwrap().try_into()?;
        tc_ingress.load()?;
        tc_ingress.attach(&self.iface, TcAttachType::Ingress)?;

        let tc_egress: &mut SchedClassifier = bpf.program_mut("tc_egress").unwrap().try_into()?;
        tc_egress.load()?;
        tc_egress.attach(&self.iface, TcAttachType::Egress)?;

        let mut network_info: HashMap<_, u8, NetworkInfo> =
            HashMap::try_from(bpf.map_mut("NETWORK_INFO").unwrap())?;

        let host_ip_info = NetworkInfo {
            ip: host_ip.parse::<Ipv4Addr>()?.into(),
            subnet_mask: 0,
        };

        let parts: Vec<&str> = cluster_cidr.split('/').collect();
        let cidr_bits = parts[1].parse::<u32>()?;

        let cluster_cidr_info = NetworkInfo {
            ip: parts[0].parse::<Ipv4Addr>()?.into(),
            subnet_mask: u32::MAX << (32 - cidr_bits),
        };

        network_info.insert(HOST_IP_KEY, host_ip_info, 0)?;
        network_info.insert(CLUSTER_CIDR_KEY, cluster_cidr_info, 0)?;

        api_server::start(pod_cidr, store_path, shutdown)
            .await
            .unwrap();

        Ok(())
    }
}
