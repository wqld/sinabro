# Netlink

Rust implementations of `rtnetlink` related features to be used by the Sinabro CNI Plugin.

List of commands to be implemented:

- ip link add peer0 type veth peer name $host_if_name
- ip link set $host_if_name up
- ip link set $host_if_name master cni0
- ip link set peer0 netns $CNI_CONTAINERID
- ip netns exec $CNI_CONTAINERID ip link set peer0 name $CNI_IFNAME
- ip netns exec $CNI_CONTAINERID ip link set $CNI_IFNAME up
- ip netns exec $CNI_CONTAINERID ip addr add $container_ip/$subnet_mask_size dev $CNI_IFNAME
- ip netns exec $CNI_CONTAINERID ip route add default via $gw_ip dev $CNI_IFNAME
- ip netns exec $CNI_CONTAINERID ip link show eth0
- ip netns exec $CNI_CONTAINERID ip addr show eth0
