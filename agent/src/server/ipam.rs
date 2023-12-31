use std::{
    collections::BTreeSet,
    net::IpAddr,
    sync::{Arc, Mutex},
};

use axum::extract::FromRef;
use ipnet::IpNet;

use super::state::AppState;

// TODO: abstract this to a trait
#[derive(Clone)]
pub struct Ipam {
    pub ip_store: Arc<Mutex<BTreeSet<IpAddr>>>,
    pub store_path: String,
}

impl Ipam {
    pub fn new(pod_cidr: &str, store_path: &str) -> Self {
        let ip_store = Arc::new(Mutex::new(Self::load(store_path).unwrap_or_else(|| {
            pod_cidr
                .parse::<IpNet>()
                .map(|subnet| subnet.hosts().skip(1).collect::<BTreeSet<IpAddr>>())
                .unwrap_or_else(|_| BTreeSet::new())
        })));

        Self {
            ip_store,
            store_path: store_path.to_owned(),
        }
    }

    fn load(store_path: &str) -> Option<BTreeSet<IpAddr>> {
        if std::path::Path::new(store_path).exists() {
            let data = std::fs::read_to_string(store_path).ok()?;
            let ip_store = data
                .lines()
                .map(|ip| ip.parse::<IpAddr>().unwrap())
                .collect::<BTreeSet<IpAddr>>();
            Some(ip_store)
        } else {
            None
        }
    }

    pub fn pop_first(&self) -> Option<String> {
        self.ip_store
            .lock()
            .unwrap()
            .pop_first()
            .map(|ip| ip.to_string())
    }

    pub fn insert(&self, ip: &str) {
        self.ip_store
            .lock()
            .unwrap()
            .insert(ip.parse::<IpAddr>().unwrap());
    }

    pub fn flush(&self) -> anyhow::Result<()> {
        let ip_store = self.ip_store.lock().unwrap();
        let data = ip_store
            .iter()
            .map(|ip| ip.to_string())
            .collect::<Vec<String>>()
            .join("\n");

        let path = std::path::Path::new(&self.store_path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }

        std::fs::write(path, data)?;
        Ok(())
    }

    #[cfg(test)]
    pub fn count(&self) -> usize {
        self.ip_store.lock().unwrap().len()
    }
}

impl FromRef<AppState> for Ipam {
    fn from_ref(state: &AppState) -> Self {
        state.ipam.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipam() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let store_path = tmp_dir.path().join("ip_store");
        println!("store_path: {:?}", store_path);
        let ipam = Ipam::new("10.244.0.0/24", store_path.to_str().unwrap());

        assert!(!std::path::Path::new(store_path.to_str().unwrap()).exists());
        assert_eq!(ipam.count(), 253);

        let addr = ipam.pop_first().unwrap();
        assert_eq!(addr, "10.244.0.2");
        let addr = ipam.pop_first().unwrap();
        assert_eq!(addr, "10.244.0.3");
        let addr = ipam.pop_first().unwrap();
        assert_eq!(addr, "10.244.0.4");
        assert_eq!(ipam.count(), 250);

        ipam.insert("10.244.0.3");
        assert_eq!(ipam.count(), 251);

        let addr = ipam.pop_first().unwrap();
        assert_eq!(addr, "10.244.0.3");

        let result = ipam.flush();
        assert!(result.is_ok());

        assert!(std::path::Path::new(store_path.to_str().unwrap()).exists());
        let data = std::fs::read_to_string(store_path.to_str().unwrap()).unwrap();
        assert_eq!(data.lines().count(), ipam.count());

        let ipam = Ipam::new("10.244.0.0/24", store_path.to_str().unwrap());
        assert_eq!(ipam.count(), 250);

        let addr = ipam.pop_first().unwrap();
        assert_eq!(addr, "10.244.0.5");
    }
}
