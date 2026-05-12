//! Long-term node identity.
//!
//! On first run, [`Identity::load_or_generate`] writes a fresh Ed25519
//! keypair to `<state_dir>/identity.key` (raw 32-byte secret followed
//! by 32-byte public key, i.e. 64 bytes total) with `0o600` perms on
//! Unix. We deliberately don't use PEM/PKCS8 here to avoid pulling in
//! another crate; the file format is a private detail of borderless.

use crate::{Error, Result};
use borderless_core::NodeId;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey, SECRET_KEY_LENGTH};
use rand::rngs::OsRng;
use std::fs;
use std::path::{Path, PathBuf};

/// Long-term Ed25519 keypair representing a node.
#[derive(Clone)]
pub struct Identity {
    signing: SigningKey,
}

impl Identity {
    /// Generate a brand-new identity. Caller is responsible for
    /// persisting it.
    pub fn generate() -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        Self { signing }
    }

    /// Load the identity from disk, creating one if missing.
    pub fn load_or_generate(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            Self::load(path)
        } else {
            let id = Self::generate();
            id.save(path)?;
            Ok(id)
        }
    }

    /// Load from an existing file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = fs::read(path)?;
        if bytes.len() < SECRET_KEY_LENGTH {
            return Err(Error::Identity("identity file too short".into()));
        }
        let mut secret = [0u8; SECRET_KEY_LENGTH];
        secret.copy_from_slice(&bytes[..SECRET_KEY_LENGTH]);
        let signing = SigningKey::from_bytes(&secret);
        Ok(Self { signing })
    }

    /// Persist to disk. On Unix, sets 0o600 permissions.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path: PathBuf = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(&self.signing.to_bytes());
        bytes.extend_from_slice(self.signing.verifying_key().as_bytes());
        fs::write(&path, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = fs::metadata(&path)?.permissions();
            perm.set_mode(0o600);
            fs::set_permissions(&path, perm)?;
        }
        Ok(())
    }

    /// Public key bytes.
    pub fn pubkey(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    /// Verifying half (for use with [`verify_signature`]).
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    /// Borderless [`NodeId`] derived from the public key.
    pub fn node_id(&self) -> NodeId {
        NodeId::from_pubkey(&self.pubkey())
    }

    /// Sign arbitrary bytes (typically a TLS exporter binding).
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.signing.sign(message).to_bytes()
    }
}

/// Verify an Ed25519 signature.
pub fn verify_signature(pubkey: &[u8; 32], message: &[u8], sig: &[u8; 64]) -> Result<()> {
    let vk = VerifyingKey::from_bytes(pubkey)
        .map_err(|e| Error::Identity(format!("bad pubkey: {e}")))?;
    let signature = Signature::from_bytes(sig);
    vk.verify(message, &signature)
        .map_err(|e| Error::Identity(format!("signature: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sign_and_verify_roundtrip() {
        let id = Identity::generate();
        let msg = b"hello tls exporter";
        let sig = id.sign(msg);
        verify_signature(&id.pubkey(), msg, &sig).unwrap();
    }

    #[test]
    fn save_and_load_yields_same_pubkey() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("id.key");
        let id1 = Identity::generate();
        id1.save(&path).unwrap();
        let id2 = Identity::load(&path).unwrap();
        assert_eq!(id1.pubkey(), id2.pubkey());
        assert_eq!(id1.node_id(), id2.node_id());
    }
}
