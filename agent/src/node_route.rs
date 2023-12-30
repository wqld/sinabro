use k8s_openapi::api::core::v1::Node;

#[derive(Debug)]
pub struct NodeRoute {
    pub ip: String,
    pub pod_cidr: String,
}

impl From<Node> for NodeRoute {
    fn from(node: Node) -> Self {
        let ip = node
            .status
            .and_then(|status| status.addresses)
            .and_then(|addresses| addresses.first().cloned())
            .map(|address| address.address)
            .unwrap_or_default();
        let pod_cidr = node.spec.and_then(|spec| spec.pod_cidr).unwrap_or_default();

        Self { ip, pod_cidr }
    }
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::{Node, NodeAddress, NodeSpec, NodeStatus};

    use super::*;

    #[test]
    fn test_node_route_from() {
        let node = Node {
            spec: Some(NodeSpec {
                pod_cidr: Some("10.244.0.0/24".to_string()),
                ..Default::default()
            }),
            status: Some(NodeStatus {
                addresses: Some(vec![NodeAddress {
                    address: "172.18.0.3".to_string(),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let node_route = NodeRoute::from(node);

        assert_eq!(node_route.ip, "172.18.0.3");
        assert_eq!(node_route.pod_cidr, "10.244.0.0/24");
    }
}
