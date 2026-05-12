//! Trust-on-first-use peer store, persisted as TOML.
//!
//! Format (`known_peers.toml`):
//!
//! ```toml
//! [[peer]]
//! name = "macbook"
//! pubkey = "2c8a..."   # 64 hex chars
//! paired_at = 1700000000
//! ```

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// One trusted peer entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KnownPeer {
    /// Human-readable name as seen at pair time.
    pub name: String,
    /// Hex-encoded Ed25519 public key.
    pub pubkey: String,
    /// Unix timestamp of pairing.
    pub paired_at: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct StoreFile {
    #[serde(default, rename = "peer")]
    peers: Vec<KnownPeer>,
}

/// In-memory + on-disk view of trusted peers.
#[derive(Debug, Default)]
pub struct PeerStore {
    path: PathBuf,
    by_pubkey: HashMap<[u8; 32], KnownPeer>,
}

impl PeerStore {
    /// Open the store at `path`, creating an empty file if missing.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path: PathBuf = path.as_ref().to_path_buf();
        let by_pubkey = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            let parsed: StoreFile = toml::from_str(&raw).map_err(|e| Error::Toml(e.to_string()))?;
            parsed
                .peers
                .into_iter()
                .filter_map(|p| {
                    let mut k = [0u8; 32];
                    let bytes = hex::decode(&p.pubkey).ok()?;
                    if bytes.len() != 32 {
                        return None;
                    }
                    k.copy_from_slice(&bytes);
                    Some((k, p))
                })
                .collect()
        } else {
            HashMap::new()
        };
        Ok(Self { path, by_pubkey })
    }

    /// True if we have already paired with `pubkey`.
    pub fn is_known(&self, pubkey: &[u8; 32]) -> bool {
        self.by_pubkey.contains_key(pubkey)
    }

    /// Look up a paired peer.
    pub fn get(&self, pubkey: &[u8; 32]) -> Option<&KnownPeer> {
        self.by_pubkey.get(pubkey)
    }

    /// Number of trusted peers.
    pub fn len(&self) -> usize {
        self.by_pubkey.len()
    }

    /// True if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.by_pubkey.is_empty()
    }

    /// All peers, in unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = &KnownPeer> {
        self.by_pubkey.values()
    }

    /// Add a new peer and persist.
    pub fn insert(&mut self, pubkey: [u8; 32], name: String, paired_at: u64) -> Result<()> {
        let peer = KnownPeer {
            name,
            pubkey: hex::encode(pubkey),
            paired_at,
        };
        self.by_pubkey.insert(pubkey, peer);
        self.flush()
    }

    /// Remove a peer and persist.
    pub fn remove(&mut self, pubkey: &[u8; 32]) -> Result<()> {
        self.by_pubkey.remove(pubkey);
        self.flush()
    }

    fn flush(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = StoreFile {
            peers: self.by_pubkey.values().cloned().collect(),
        };
        let raw = toml::to_string_pretty(&file).map_err(|e| Error::Toml(e.to_string()))?;
        fs::write(&self.path, raw)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trip_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("known_peers.toml");

        let pk = [3u8; 32];
        {
            let mut store = PeerStore::open(&path).unwrap();
            assert!(store.is_empty());
            store.insert(pk, "alice".into(), 1_700_000_000).unwrap();
            assert!(store.is_known(&pk));
        }
        let store = PeerStore::open(&path).unwrap();
        assert_eq!(store.len(), 1);
        assert_eq!(store.get(&pk).unwrap().name, "alice");
    }
}
