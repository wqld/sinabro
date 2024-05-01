build-image:
    cargo xtask build-ebpf
    cargo build --target $(uname -m)-unknown-linux-musl
    docker build --build-arg ARCH=$(uname -m) -t sinabro:test .

setup-kind-cluster: build-image
    kind create cluster --config tests/e2e/kind-config.yaml
    kind load docker-image sinabro:test

delete-kind-cluster:
    kind delete cluster

create-kind-cluster-with-sinabro: setup-kind-cluster
    kubectl apply -f tests/e2e/deploy-test/agent.yaml

deploy-test-pods:
    kubectl taint nodes kind-control-plane node-role.kubernetes.io/control-plane-
    kubectl apply -f tests/e2e/deploy-test/test-pods.yaml

cargo-check:
    cargo fmt --all -- --check
    cargo clippy --all --all-targets --all-features -- -D warnings
    cargo test --all --lib --bins --tests --examples --all-features

e2e-test: build-image
    kubectl kuttl test --config ./tests/kuttl-test.yaml

launch-rust-env:
    docker run --rm --privileged -it -v $(pwd):/source rust sh

run-rsb:
    kubectl run rsb --image gamelife1314/rsb -- -d 30 -c 500 -l http://$(kubectl get pod nginx-worker -o jsonpath='{.status.podIP}')