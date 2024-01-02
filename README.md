# Sinabro

Sinabro is a networking, observability and security solution with an eBPF and WASM-based data plane written in Rust.

## Getting Started

Currently, Sinabro is in the early stages of development. I am progressively developing it in the following environment, which is also the verified execution environment.

- M3 Max MacBook Pro (devcontainer)
- rustup 1.26.0 / rustc 1.75.0
- Docker version 24.0.7
- kind v0.20.0

Please note that as the project is still in its infancy, there may be certain limitations or issues that have not yet been fully addressed. Your understanding and patience are greatly appreciated.

### Prerequisites

- Rust (aya)
- Docker
- Kubectl
- Kind

### start kind cluster

When starting the kind cluster, the default kindnet CNI must be disabled in order to verify the operation of the Sinabro CNI. Start the cluster using the predefined config related to this:

```bash
kind create cluster --config test/kind-config.yaml
```

### build

In a Linux environment, you can build the eBPF program and the userspace application, known as the agent, using the following commands. (I have set up my development environment using a devcontainer.)

```bash
cargo xtask build-ebpf
cargo build --target aarch64-unknown-linux-musl
```

After compiling, build the container image and load it into the kind cluster:

```bash
docker build -t sinabro:0.0.1 .
kind load docker-image sinabro:0.0.1
```

### deploy

Deploy the Sinabro CNI to the kind cluster:

```bash
kubectl apply -f test/agent.yaml
```

## features (still in development)

Only networking within the same network for containers is supported.
