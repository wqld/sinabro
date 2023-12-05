use serde::Serialize;

#[derive(Serialize)]
pub struct CniConfig<'a> {
    #[serde(rename = "cniVersion")]
    pub cni_version: &'a str,

    pub name: &'a str,

    #[serde(rename = "type")]
    pub cni_type: &'a str,

    pub network: &'a str,

    pub subnet: &'a str,
}

impl CniConfig<'_> {
    pub fn new<'a>(network: &'a str, subnet: &'a str) -> CniConfig<'a> {
        CniConfig {
            cni_version: "0.3.1",
            name: "sinabro",
            cni_type: "sinabro-cni",
            network,
            subnet,
        }
    }

    pub fn write(&self, path: &str) -> anyhow::Result<()> {
        let json = serde_json::to_string(self)?;

        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, json).map_err(|e| anyhow::anyhow!(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_cni_config() {
        let cluster_cidr = "10.244.0.0/16";
        let pod_cidr = "10.244.0.0/24";

        CniConfig::new(cluster_cidr, pod_cidr)
            .write("/tmp/10-sinabro.conf")
            .unwrap();

        let expected = r#"{"cniVersion":"0.3.1","name":"sinabro","type":"sinabro-cni","network":"10.244.0.0/16","subnet":"10.244.0.0/24"}"#;
        let json = std::fs::read_to_string("/tmp/10-sinabro.conf").unwrap();
        std::fs::remove_file("/tmp/10-sinabro.conf").unwrap();

        assert_eq!(expected, json);
    }
}
