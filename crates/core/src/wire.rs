//! Top-level wire frames carried over the TCP+TLS stream.
//!
//! v0.2 carries every kind of message — input, clipboard, control,
//! lazy-payload fetch — over a single bidirectional TLS stream framed
//! with a 4-byte little-endian length prefix followed by a postcard
//! payload. There are no QUIC datagrams in v0.2; head-of-line latency
//! on a LAN-scoped TCP connection is well under the perceptual budget
//! for cursor and keystroke events.

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
    /// Input event. v0.2: only Hub -> Spoke; spokes that send input
    /// frames are protocol-violating and the hub drops them.
    Input(InputEvent),
    /// Clipboard update. Bidirectional; the hub fans out spoke
    /// updates to every other spoke.
    Clipboard(ClipboardSnapshot),
    /// Request the bytes of an out-of-line `LazyPayload` by hash.
    FetchRequest {
        /// BLAKE3 hash of the payload, used as the fetch key.
        hash: [u8; 32],
    },
    /// Reply to a [`WireFrame::FetchRequest`]. Large payloads are
    /// chunked: receivers concatenate `bytes` across responses with
    /// matching `hash` and `chunk_idx ∈ 0..total`.
    FetchResponse {
        /// Hash being delivered.
        hash: [u8; 32],
        /// Zero-based chunk index.
        chunk_idx: u32,
        /// Total number of chunks (so the receiver knows when to stop).
        total: u32,
        /// Chunk bytes.
        bytes: Vec<u8>,
    },
    /// Sent in reply to a `FetchRequest` for which we have nothing to
    /// serve (peer asked for an unknown hash).
    FetchMiss {
        /// Hash that wasn't found.
        hash: [u8; 32],
    },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::{ClipItem, ClipboardSnapshot};

    fn roundtrip<T>(v: &T)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let bytes = crate::encode(v).unwrap();
        let back: T = crate::decode(&bytes).unwrap();
        assert_eq!(v, &back);
    }

    #[test]
    fn fetch_frames_round_trip() {
        let h = [7u8; 32];
        roundtrip(&WireFrame::FetchRequest { hash: h });
        roundtrip(&WireFrame::FetchResponse {
            hash: h,
            chunk_idx: 3,
            total: 9,
            bytes: vec![1, 2, 3, 4, 5],
        });
        roundtrip(&WireFrame::FetchMiss { hash: h });
    }

    #[test]
    fn clipboard_frame_round_trips() {
        let snap = ClipboardSnapshot {
            version: 42,
            origin: NodeId([0xAB; 16]),
            created_unix_ms: 0,
            items: vec![ClipItem::Text("hello".into())],
        };
        roundtrip(&WireFrame::Clipboard(snap));
    }

    #[test]
    fn control_frames_round_trip() {
        roundtrip(&WireFrame::Control(ControlFrame::Hello {
            node_id: NodeId([1; 16]),
            name: "hub".into(),
            max_protocol: ProtocolVersion(0),
        }));
        roundtrip(&WireFrame::Control(ControlFrame::Ping { nonce: 0xCAFE }));
        roundtrip(&WireFrame::Control(ControlFrame::Bye {
            reason: "test".into(),
        }));
    }
}
