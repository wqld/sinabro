use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
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

impl<'a> From<&'a str> for CniConfig<'a> {
    fn from(json: &'a str) -> Self {
        serde_json::from_str(json).unwrap()
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

    #[test]
    fn cni_config_from_json() {
        let json = r#"{"cniVersion":"0.3.1","name":"sinabro","type":"sinabro-cni","network":"10.244.0.0/16","subnet":"10.244.0.0/24"}"#;
        let cni_config = CniConfig::from(json);

        assert_eq!("0.3.1", cni_config.cni_version);
        assert_eq!("sinabro", cni_config.name);
        assert_eq!("sinabro-cni", cni_config.cni_type);
        assert_eq!("10.244.0.0/16", cni_config.network);
        assert_eq!("10.244.0.0/24", cni_config.subnet);
    }
}
