//! Node identity and protocol versioning.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable opaque identifier for a node, derived from its long-term
/// Ed25519 public key (BLAKE3-truncated).
///
/// 16 bytes is plenty given a LAN with O(10) peers.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub [u8; 16]);

impl NodeId {
    /// Hash a 32-byte Ed25519 public key down to a 16-byte node id.
    pub fn from_pubkey(pubkey: &[u8; 32]) -> Self {
        let h = blake3::hash(pubkey);
        let mut out = [0u8; 16];
        out.copy_from_slice(&h.as_bytes()[..16]);
        Self(out)
    }

    /// Hex string representation.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", self.to_hex())
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// Wire-protocol version. We bump this when introducing breaking
/// changes; the negotiating side picks the highest mutually supported
/// version in the `Hello` frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion(pub u16);

/// Initial protocol version shipped with v0.1 MVP.
pub const PROTOCOL_V0: ProtocolVersion = ProtocolVersion(0);
