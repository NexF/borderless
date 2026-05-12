//! Top-level wire frames carried over QUIC streams and datagrams.

use crate::clipboard::ClipboardSnapshot;
use crate::input::InputEvent;
use crate::node::{NodeId, ProtocolVersion};
use serde::{Deserialize, Serialize};

/// The root enum every postcard-encoded frame is wrapped in.
///
/// Versioning strategy: when v1 ships, this becomes `WireV1` and
/// transports negotiate the highest mutually supported version in
/// `ControlFrame::Hello`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireFrame {
    /// Connection bring-up / tear-down / liveness.
    Control(ControlFrame),
    /// Input event (also carried via QUIC datagram for `MouseMove`).
    Input(InputEvent),
    /// Clipboard update.
    Clipboard(ClipboardSnapshot),
}

/// Connection-level signalling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlFrame {
    /// First frame on a new connection; advertises identity and
    /// protocol-version preferences.
    Hello {
        /// Self id (BLAKE3 of pubkey).
        node_id: NodeId,
        /// Human-readable name (`hostname` by default).
        name: String,
        /// Highest protocol version this peer can speak.
        max_protocol: ProtocolVersion,
    },
    /// Reply to `Hello`. Settles the negotiated version.
    Welcome {
        /// Self id.
        node_id: NodeId,
        /// Self name.
        name: String,
        /// Negotiated protocol version.
        protocol: ProtocolVersion,
    },
    /// Liveness probe. Every 5 s by default.
    Ping {
        /// Echoed back in `Pong`.
        nonce: u64,
    },
    /// Liveness reply.
    Pong {
        /// Same nonce as in `Ping`.
        nonce: u64,
    },
    /// Graceful disconnect.
    Bye {
        /// Optional reason for logs.
        reason: String,
    },
}
