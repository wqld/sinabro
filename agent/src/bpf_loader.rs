use std::sync::Arc;

use aya::programs::{tc, SchedClassifier, TcAttachType};
use aya::{include_bytes_aligned, Bpf};
use aya_log::BpfLogger;
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
        let program: &mut SchedClassifier = bpf.program_mut("tc_egress").unwrap().try_into()?;
        program.load()?;
        program.attach(&self.iface, TcAttachType::Egress)?;

        api_server::start(&pod_cidr, store_path, shutdown)
            .await
            .unwrap();

        Ok(())
    }
}
