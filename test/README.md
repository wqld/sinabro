# Bash CNI example

```shell
kind create cluster --config kind-config.yaml

docker ps
docker exec -it <container-id> sh

# -- inside each node's container
apt update && apt install -y nmap

# create and configure the bridge with the cni0 name
ip link add cni0 type bridge
ip link set cni0 up
ip addr add <bridge-ip>/24 dev cni0
ip route | grep cni0

# apply additional forwarding rules that will allow
# to freely forward traffic inside the whole pod CIDR range
iptables -t filter -A FORWARD -s 10.244.0.0/16 -j ACCEPT
iptables -t filter -A FORWARD -d 10.244.0.0/16 -j ACCEPT

# setup a network address translation (NAT)
iptables -t nat -A POSTROUTING -s <pod-cidr> ! -o cni0 -j MASQUERADE

# setup additional route rule
ip route add <other-side-pod-cidr via <other-side-node-ip> dev eth0

chmod +x comet-cni && cp comet-cni /opt/cni/bin/comet-cni
# -- inside each node's container

kubectl taint nodes kind-control-plane node-role.kubernetes.io/control-plane-

kubectl apply -f test-deploy.yaml

# -- inside bash-worker
ping <nginx-worker-container-ip>
ping <nginx-master-container-ip>
# -- inside bash-worker
```