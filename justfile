setup-kind-cluster:
    kind create cluster --config tests/e2e/kind-config.yaml
    cargo xtask build-ebpf
    cargo build --target $(uname -m)-unknown-linux-musl
    docker build --build-arg ARCH=$(uname -m) -t sinabro:test .
    kind load docker-image sinabro:test

clean-kind-cluster:
    kind delete cluster

deploy-agent: setup-kind-cluster
    kubectl apply -f tests/e2e/deploy-test/00-install.yaml

deploy-test-pods: deploy-agent
    kubectl taint nodes kind-control-plane node-role.kubernetes.io/control-plane-
    kubectl apply -f tests/e2e/deploy-test/01-test-pods.yaml
