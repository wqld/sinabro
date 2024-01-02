use anyhow::Context;
use aya::programs::{Xdp, XdpFlags};
use aya::{include_bytes_aligned, Bpf};
use aya_log::BpfLogger;
use tracing::{debug, warn};

pub struct BpfLoader {
    iface: String,
}

impl BpfLoader {
    pub fn new(iface: &str) -> Self {
        Self {
            iface: iface.to_string(),
        }
    }

    pub fn load(&self) -> anyhow::Result<()> {
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
        bpf.programs().for_each(|(a, _)| debug!("program: {:?}", a));
        let program: &mut Xdp = bpf.program_mut("xdp").unwrap().try_into()?;
        program.load()?;
        program
            .attach(&self.iface, XdpFlags::SKB_MODE)
            .context("failed to attach the XDP program")?;

        Ok(())
    }
}
