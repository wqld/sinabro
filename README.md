# Sinabro

Sinabro is a networking solution for Kubernetes that leverages eBPF to provide high-performance networking and security features.

## Components

- **[agent](https://github.com/wqld/sinabro/tree/main/agent)**: The Sinabro agent is a daemon that runs on each node in the Kubernetes cluster. It is responsible for managing the network interfaces and routing tables required by the Sinabro CNI. For high performance, it utilizes eBPF programs.
- **[cni](https://github.com/wqld/sinabro/tree/main/cni)**: The Sinabro CNI is a container network interface plugin that is responsible for setting up the network interfaces and routing tables required by the pods in the Kubernetes cluster.

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

### TCP Acceleration

An eBPF program has been applied to accelerate TCP transmission between pods communicating on the same host machine. This avoids unnecessary traversing through the Linux network stack, enabling efficient communication between local socket pairs.

#### Without eBPF Acceleration

```sh
Get "http://10.244.1.2" with for 10s using 50 connections
Statistics         Avg          Stdev          Max
  Reqs/sec       77688.89      1898.47       80410.00
  Latency        592.08µs      411.46µs       8.69ms
  Latency Distribution
     50%     309.77µs
     75%     409.98µs
     90%     490.44µs
     99%     571.86µs
  HTTP codes:
    1XX - 0, 2XX - 777807, 3XX - 0, 4XX - 0, 5XX - 0
    others - 0
  Throughput:   84447.90/s

Get "http://10.244.1.2" with for 30s using 500 connections
Statistics         Avg          Stdev          Max
  Reqs/sec       73050.03      1374.04       75450.00
  Latency        791.10µs      822.26µs      54.64ms
  Latency Distribution
     50%     361.55µs
     75%     503.02µs
     90%     622.58µs
     99%     749.10µs
  HTTP codes:
    1XX - 0, 2XX - 2192021, 3XX - 0, 4XX - 0, 5XX - 0
    others - 0
  Throughput:  632031.35/s
```

#### With eBPF Acceleration

```sh
Get "http://10.244.1.2" with for 10s using 50 connections
Statistics         Avg          Stdev          Max
  Reqs/sec       81633.44      1638.01       84030.00
  Latency        539.51µs      366.21µs      11.88ms
  Latency Distribution
     50%     285.54µs
     75%     377.27µs
     90%     449.91µs
     99%     521.56µs
  HTTP codes:
    1XX - 0, 2XX - 812374, 3XX - 0, 4XX - 0, 5XX - 0
    others - 0
  Throughput:   92676.69/s

Get "http://10.244.1.2" with for 30s using 500 connections
Statistics         Avg          Stdev          Max
  Reqs/sec       76810.21      1745.58       79262.00
  Latency        650.09µs      714.47µs      61.40ms
  Latency Distribution
     50%     305.78µs
     75%     422.25µs
     90%     518.34µs
     99%     616.42µs
  HTTP codes:
    1XX - 0, 2XX - 2305881, 3XX - 0, 4XX - 0, 5XX - 0
    others - 0
  Throughput:  769121.91/s
```

Tests were conducted with 50 connections for 10 seconds and 500 connections for 30 seconds.
The results indicate an increase in the average request rate and throughput, and a decrease in latency.

| Test Case | Requests/sec | Throughput | Latency |
| --- | --- | --- | --- |
| Without Acceleration (10s) | 77688.89 | 84447.90/s | 592.08µs |
| With Acceleration (10s) | 81633.44 | 92676.69/s | 539.51µs |
| Without Acceleration (30s) | 73050.03 | 632031.35/s | 791.10µs |
| With Acceleration (30s) | 76810.21 | 769121.91/s | 650.09µs |
