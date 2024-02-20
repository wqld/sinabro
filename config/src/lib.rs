use std::path::Path;

use anyhow::{anyhow, Result};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::level_filters::LevelFilter;
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::fmt;

#[derive(Serialize, Deserialize)]
pub struct Config<'a> {
    #[serde(rename = "cniVersion")]
    pub cni_version: &'a str,

    pub name: &'a str,

    #[serde(rename = "type")]
    pub cni_type: &'a str,

    pub network: &'a str,

    pub subnet: &'a str,
}

impl Config<'_> {
    pub fn new<'a>(network: &'a str, subnet: &'a str) -> Config<'a> {
        Config {
            cni_version: "0.3.1",
            name: "sinabro",
            cni_type: "sinabro-cni",
            network,
            subnet,
        }
    }

    pub fn write(&self, path: &str) -> Result<()> {
        let json = serde_json::to_string(self)?;

        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, json).map_err(|e| anyhow!(e))
    }
}

impl<'a> From<&'a str> for Config<'a> {
    fn from(json: &'a str) -> Self {
        serde_json::from_str(json).unwrap()
    }
}

pub fn setup_tracing_to_stdout(filter: impl Into<LevelFilter>) {
    fmt().with_max_level(filter).init();
}

pub fn setup_tracing_to_file(
    directory: impl AsRef<Path>,
    file_name_prefix: impl AsRef<Path>,
    filter: impl Into<LevelFilter>,
) -> Result<non_blocking::WorkerGuard> {
    let file_appender = rolling::daily(directory, file_name_prefix);
    let (non_blocking, guard) = non_blocking(file_appender);
    fmt()
        .with_writer(non_blocking)
        .with_max_level(filter)
        .init();

    Ok(guard)
}

pub fn generate_mac_addr() -> Result<Vec<u8>> {
    let mut rng = rand::thread_rng();
    let mut buf = [0u8; 6];
    rng.fill(&mut buf[..]);

    buf[0] = (buf[0] | 0x02) & 0xfe;

    Ok(buf.to_vec())
}

pub fn parse_mac(mac: &str) -> Result<Vec<u8>> {
    let mac = mac
        .split(':')
        .map(|s| u8::from_str_radix(s, 16))
        .collect::<Result<Vec<u8>, _>>()?;

    if mac.len() != 6 {
        return Err(anyhow!("Invalid MAC address"));
    }

    Ok(mac)
}

#[cfg(test)]
mod tests {
    use tracing::Level;

    use super::*;

    #[test]
    fn write_config() {
        let cluster_cidr = "10.244.0.0/16";
        let pod_cidr = "10.244.0.0/24";

        Config::new(cluster_cidr, pod_cidr)
            .write("/tmp/10-sinabro.conf")
            .unwrap();

        let expected = r#"{"cniVersion":"0.3.1","name":"sinabro","type":"sinabro-cni","network":"10.244.0.0/16","subnet":"10.244.0.0/24"}"#;
        let json = std::fs::read_to_string("/tmp/10-sinabro.conf").unwrap();
        std::fs::remove_file("/tmp/10-sinabro.conf").unwrap();

        assert_eq!(expected, json);
    }

    #[test]
    fn config_from_json() {
        let json = r#"{"cniVersion":"0.3.1","name":"sinabro","type":"sinabro-cni","network":"10.244.0.0/16","subnet":"10.244.0.0/24"}"#;
        let cni_config = Config::from(json);

        assert_eq!("0.3.1", cni_config.cni_version);
        assert_eq!("sinabro", cni_config.name);
        assert_eq!("sinabro-cni", cni_config.cni_type);
        assert_eq!("10.244.0.0/16", cni_config.network);
        assert_eq!("10.244.0.0/24", cni_config.subnet);
    }

    #[tokio::test]
    async fn test_setup_tracing_to_file() {
        let _guard = setup_tracing_to_file("/tmp", "sinabro.log", Level::DEBUG).unwrap();
        tracing::debug!("Hello, world!");

        let current_date = chrono::Local::now().format("%Y-%m-%d");
        let file_name = format!("/tmp/sinabro.log.{}", current_date);
        assert!(Path::new(&file_name).exists());

        let file_content = std::fs::read_to_string(&file_name).unwrap();
        println!("{}", file_content);
        assert!(file_content.contains("Hello, world!"));

        std::fs::remove_file(&file_name).unwrap();
    }

    #[test]
    fn test_generate_mac_addr() {
        let mac_addr = generate_mac_addr().unwrap();
        assert_eq!(mac_addr.len(), 6);
        assert_eq!(mac_addr[0] & 0x01, 0);
        assert_eq!(mac_addr[0] & 0x02, 2);
    }

    #[test]
    fn test_parse_mac_valid() {
        let mac_str = "aa:bb:cc:dd:00:01";
        let mac_addr = parse_mac(mac_str).unwrap();
        assert_eq!(mac_addr, vec![0xaa, 0xbb, 0xcc, 0xdd, 0x00, 0x01]);
    }

    #[test]
    fn test_parse_mac_invalid_length() {
        let mac_str = "aa:bb:cc:dd:00";
        let result = parse_mac(mac_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_mac_invalid_chars() {
        let mac_str = "aa:bb:cc:dd:00:ZZ";
        let result = parse_mac(mac_str);
        assert!(result.is_err());
    }
}
