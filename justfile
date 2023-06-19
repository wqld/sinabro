[private]
default:
  @just --list --unsorted

# install crd into the cluster
install-crd: generate
  kubectl apply -f yaml/crd.yaml

generate:
  cargo run --bin crdgen > yaml/crd.yaml
  helm template charts/agent > yaml/deploy.yaml

# compile for musl (for docker image)
# cargo build --release --features={{features}} --bin agent
compile features="":
  #!/usr/bin/env bash
  docker run --rm \
    -v cargo-cache:/root/.cargo \
    -v $PWD:/volume \
    -w /volume \
    -t rust:latest \
    sh -c "apt update && \
    apt install -y pkg-config libssl-dev musl-tools musl-dev clang llvm && \
    export CC_aarch64_unknown_linux_musl=clang && \
    export AR_aarch64_unknown_linux_musl=llvm-ar && \
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS='-Clink-self-contained=yes -Clinker=rust-lld' && \
    rustup target add aarch64-unknown-linux-musl && \
    cargo build --target aarch64-unknown-linux-musl --release --features={{features}} --bin agent"
  cp target/aarch64-unknown-linux-musl/release/agent ./agent/
