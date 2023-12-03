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
            .and_then(|addresses| addresses.get(0).cloned())
            .map(|address| address.address)
            .unwrap_or_default();
        let pod_cidr = node.spec.and_then(|spec| spec.pod_cidr).unwrap_or_default();

        Self { ip, pod_cidr }
    }
}
