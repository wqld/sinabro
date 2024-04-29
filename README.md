# Sinabro

Sinabro is a networking solution for Kubernetes that leverages eBPF to provide high-performance networking and security features.

## Components

- **[agent](https://github.com/wqld/sinabro/tree/main/agent)**: The Sinabro agent is a daemon that runs on each node in the Kubernetes cluster. It is responsible for managing the network interfaces and routing tables required by the Sinabro CNI. For high performance, it utilizes eBPF programs.
- **[cni](https://github.com/wqld/sinabro/tree/main/cni)**: The Sinabro CNI is a container network interface plugin that is responsible for setting up the network interfaces and routing tables required by the pods in the Kubernetes cluster.
- **[wgctrl](https://github.com/wqld/sinabro/tree/main/wgctrl)**: A Rust implementation of the [wgctrl-go](https://github.com/WireGuard/wgctrl-go) project, this is a WireGuard control library that provides the WireGuard control interface required by the Sinabro project.

## Getting Started

Sinabro is currently in its early stages of development. The ongoing development is being carried out in the following environment, which is also the verified execution environment:

- Ubuntu 22.04 arm64 on UTM

Please note that due to the project's infancy, there may be limitations or issues that have not yet been fully addressed. Your understanding and patience are greatly appreciated.

### Prerequisites

- [Rust](https://www.rust-lang.org) ([Aya](https://aya-rs.dev))
- [Docker](https://www.docker.com) ([Kind](https://kind.sigs.k8s.io))
- [Kubectl](https://kubernetes.io/docs/reference/kubectl/)
- [Just](https://just.systems)

### Starting a kind cluster and deploying Sinabro CNI

To verify the operation of the Sinabro CNI, the default kindnet CNI must be disabled when starting the kind cluster. Start the cluster using the predefined config related to this:

```bash
just deploy-agent
```

## features (still in development)

- [x] IPAM: IP addresses are currently managed based on files. Further implementation is planned for managing IP addresses through Kubernetes' CRD.
- [x] eBPF-based Masquerading: NAT has been set up using eBPF to manage traffic exiting the cluster, with further enhancements in progress.
- [x] Sinabro-specific Netlink Library: A separate netlink library has been created to facilitate the addition and modification of the netlink features required by Sinabro.
- [x] VxLAN Overlay Network: The current implementation uses the ARP and FDB tables of the Linux kernel. Future plans include replacing this with BPF for lightweight tunneling.
- [ ] Route Service Traffic without Kube-proxy
- [ ] Enforce Network Policies with eBPF
- [ ] Build an XDP-based BGP Peering Router
- [ ] Implement Service Load Balancing
- [ ] Collect Network Telemetry with eBPF
