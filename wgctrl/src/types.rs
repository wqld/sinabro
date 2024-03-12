use std::{
    fmt,
    net::SocketAddr,
    ops::{Deref, DerefMut},
    time::{Duration, SystemTime},
};

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use ipnet::IpNet;
use rand::Rng;
use x25519_dalek::{PublicKey, StaticSecret};

/// `KEY_LEN` is the expected key length for a WireGuard key.
const KEY_LEN: usize = 32;

/// `DeviceType` specifies the underlying implementation of a WireGuard device.
pub enum DeviceType {
    LinuxKernel,
    Userspace,
    Unknown,
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceType::LinuxKernel => write!(f, "linux-kernel"),
            DeviceType::Userspace => write!(f, "userspace"),
            DeviceType::Unknown => write!(f, "unknown"),
        }
    }
}

/// A `Key` is a public, private, or pre-shared secret key.
/// The Key constructor functions in this package can be used
/// to create Keys suitable for each of these applications.
pub struct Key([u8; KEY_LEN]);

impl TryFrom<&[u8]> for Key {
    type Error = anyhow::Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != KEY_LEN {
            return Err(anyhow!("Incorrect key size: {}", bytes.len()));
        }

        let mut key = [0; KEY_LEN];
        key.copy_from_slice(bytes);

        Ok(Self(key))
    }
}

impl TryFrom<&str> for Key {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self> {
        let bytes = general_purpose::STANDARD.decode(s)?;
        Self::try_from(bytes.as_slice())
    }
}

impl Deref for Key {
    type Target = [u8; KEY_LEN];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Key {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Key> for String {
    fn from(val: Key) -> Self {
        general_purpose::STANDARD.encode(*val)
    }
}

impl Key {
    /// `generate_key` generates a Key suitable for use as a pre-shared secret key from
    /// a cryptographically safe source.
    ///
    /// The output Key should not be used as a private key; use `generate_private_key` instead.
    pub fn generate_key() -> Result<Self> {
        let mut key = [0; KEY_LEN];
        let mut rng = rand::thread_rng();
        rng.fill(&mut key);

        Key::try_from(key.as_ref())
    }

    /// `generate_private_key` generates a Key suitable for use as a private key from a
    /// cryptographically safe source.
    pub fn generate_private_key() -> Result<Self> {
        let mut key = Key::generate_key()?;

        // Modify random bytes using algorithm described at:
        // https://cr.yp.to/ecdh.html.
        key[0] &= 248;
        key[31] &= 127;
        key[31] |= 64;

        Ok(key)
    }

    /// `public_key` computes a public key from the private key.
    pub fn public_key(&self) -> Key {
        let secret = StaticSecret::from(self.0);
        let public: PublicKey = (&secret).into();
        Self(*public.as_bytes())
    }

    /// `exchange` performs ECDH key exchange with a public key.
    pub fn exchange(&self, public_key: &Key) -> Key {
        let secret = StaticSecret::from(self.0);
        let public: PublicKey = (public_key.0).into();
        let shared = secret.diffie_hellman(&public);
        Self(*shared.as_bytes())
    }
}

/// `Peer` is a WireGuard peer to a Device.
pub struct Peer {
    /// `public_key` is the public key of a peer, computed from its private key.
    /// It is always present in a Peer.
    pub public_key: Key,

    /// `preshared_key` is an optional preshared key which may be used
    /// as an additional layer of security for peer communications.
    pub preshared_key: Option<Key>,

    /// `endpoint` is the most recent source address used for communication by this Peer.
    pub endpoint: SocketAddr,

    /// `persistent_keepalive_interval` specifies how often an "empty" packet is
    /// sent to a peer to keep a connection alive.
    pub persistent_keepalive_interval: Option<Duration>,

    /// `last_handshake_time` indicates the most recent time a handshake was performed with this peer.
    pub last_handshake_time: Option<SystemTime>,

    /// `rx_bytes` indicates the number of bytes received from this peer.
    pub rx_bytes: i64,

    /// `tx_bytes` indicates the number of bytes transmitted to this peer.
    pub tx_bytes: i64,

    /// `allowed_ips` specifies which IPv4 and IPv6 addresses this peer is allowed to communicate on.
    pub allowed_ips: Vec<IpNet>,

    /// `protocol_version` specifies which version of the WireGuard protocol is used for this Peer.
    pub protocol_version: Option<u16>,
}

/// `PeerConfig` is a WireGuard device peer configuration.
pub struct PeerConfig {
    /// `public_key` specifies the public key of this peer.
    /// PublicKey is a mandatory field for all PeerConfigs.
    pub public_key: Key,

    /// `remove` specifies if the peer with this public key should be removed
    /// from a device's peer list.
    pub remove: bool,

    /// `update_only` specifies that an operation will only occur on this peer
    /// if the peer already exists as part of the interface.
    pub update_only: bool,

    /// `preshared_key` specifies a peer's preshared key configuration, if not none.
    pub preshared_key: Option<Key>,

    /// `endpoint` specifies the endpoint of this peer entry, if not none.
    pub endpoint: Option<SocketAddr>,

    /// `persistent_keepalive_interval` specifies the persistent keepalive interval
    /// for this peer, if not none.
    pub persistent_keepalive_interval: Option<Duration>,

    /// `replace_allowed_ips` specifies if the allowed IPs specified in this peer configuration
    /// should replace any existing ones, instead of appending them to the allowed IPs list.
    pub replace_allowed_ips: bool,

    /// `allowed_ips` specifies a list of allowed IP addresses in CIDR notation for this peer.
    pub allowed_ips: Vec<IpNet>,
}

/// `Device` is a WireGuard device.
pub struct Device {
    /// `name` is the name of the device.
    pub name: String,

    /// `device_type` specifies the underlying implementation of the device.
    pub device_type: DeviceType,

    /// `private_key` is the device's private key.
    pub private_key: Key,

    /// `public_key` is the device's public key, computed from its PrivateKey.
    pub public_key: Key,

    /// `listen_port` is the device's network listening port.
    pub listen_port: u16,

    /// `firewall_mark` is the device's current firewall mark.
    ///
    /// The firewall mark can be used in conjunction with firewall software to
    /// take action on outgoing WireGuard packets.
    pub firewall_mark: u16,

    /// `peers` is the list of network peers associated with this device.
    pub peers: Vec<Peer>,
}

/// `Config` is a WireGuard device configuration.
pub struct Config {
    /// `private_key` specifies a private key configuration, if not none.
    pub private_key: Option<Key>,

    /// `listen_port` specifies a device's listening port, if not none.
    pub listen_port: Option<u16>,

    /// `firewall_mark` specifies a device's firewall mark, if not none.
    pub firewall_mark: Option<u16>,

    /// `replace_peers` specifies if the Peers in this configuration should replace
    /// the existing peer list, instead of appending them to the existing list.
    pub replace_peers: bool,

    /// `peers` specifies a list of peer configurations to apply to a device.
    pub peers: Vec<PeerConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepared_keys() {
        // Keys generated via "wg genkey" and "wg pubkey" for comparison
        // with this Rust implementation.
        let private = "GHuMwljFfqd2a7cs6BaUOmHflK23zME8VNvC5B37S3k=";
        let public = "aPxGwq8zERHQ3Q1cOZFdJ+cvJX5Ka4mLN38AyYKYF10=";

        let priv_key = Key::try_from(private).unwrap();
        let public_key = priv_key.public_key();

        assert_eq!(private, Into::<String>::into(priv_key));
        assert_eq!(public, Into::<String>::into(public_key));
    }

    #[test]
    fn test_key_exchange() {
        let alice = Key::generate_private_key().unwrap();
        let bob = Key::generate_private_key().unwrap();

        let alice_pub = alice.public_key();
        let bob_pub = bob.public_key();

        let alice_shared = alice.exchange(&bob_pub);
        let bob_shared = bob.exchange(&alice_pub);

        assert_eq!(*alice_shared, *bob_shared);
    }
}
