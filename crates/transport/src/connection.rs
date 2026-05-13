//! Authenticated, framed connection over TCP+TLS.
//!
//! Wire shape:
//!
//! 1. After the TLS handshake, both sides exchange a single
//!    [`SignedHello`] (length-prefixed, postcard-encoded) signed over
//!    the TLS exporter material. This binds the long-term Ed25519
//!    identity to the live TLS session.
//! 2. After both Hellos validate, the connection carries
//!    `WireFrame`s ([`borderless_core::WireFrame`]) framed as
//!    `u32-LE length || postcard payload`. There is exactly one
//!    bidirectional frame stream per connection (TCP, no QUIC streams).
//!
//! The [`Connection`] type wraps the underlying `TlsStream` and
//! exposes `send_frame` / `recv_frame`. All input events, clipboard
//! frames, and fetch traffic share this single stream.

use crate::identity::{verify_signature, Identity};
use crate::peer_store::PeerStore;
use crate::{Error, Result};

use borderless_core::{NodeId, ProtocolVersion, WireFrame, PROTOCOL_V0};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex as AsyncMutex;
use tokio_rustls::TlsStream;

/// Domain-separation tag mixed into the Hello signature.
pub const HELLO_BIND_LABEL: &[u8] = b"borderless/hello/v0";

/// Maximum frame payload size, applies to both Hello and WireFrame.
const MAX_FRAME: usize = 64 * 1024 * 1024;

/// Application-layer Hello, signed by the long-term Ed25519 key.
///
/// The signature covers `HELLO_BIND_LABEL || tls_exporter` so that a
/// MITM intercepting the TLS layer would still need the peer's
/// private key to forge a Hello matching their TLS session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedHello {
    /// Long-term public key of the sender.
    pub pubkey: [u8; 32],
    /// Sender's display name.
    pub name: String,
    /// Highest protocol version we can speak.
    pub max_protocol: ProtocolVersion,
    /// Ed25519 signature (64 bytes) over `HELLO_BIND_LABEL || tls_exporter`.
    pub signature: Vec<u8>,
}

/// One side of an authenticated TLS-framed session.
///
/// Cloning is intentionally not supported: the underlying TCP stream
/// is single-owner. Use `tokio::sync::Mutex` (already wrapped here)
/// to share access across tasks; readers and writers are split via
/// the inner [`tokio::io::split`] semantics implicit in the lock.
pub struct Connection {
    inner: Arc<AsyncMutex<TlsStream<tokio::net::TcpStream>>>,
    /// Verified peer pubkey (32 bytes).
    pub peer_pubkey: [u8; 32],
    /// Verified peer name (from Hello).
    pub peer_name: String,
    /// Verified peer NodeId.
    pub peer_node_id: NodeId,
}

impl Connection {
    /// Send a `WireFrame` reliably on the underlying TLS stream.
    pub async fn send_frame(&self, frame: &WireFrame) -> Result<()> {
        let bytes =
            borderless_core::encode(frame).map_err(|e| Error::Codec(format!("encode: {e}")))?;
        self.write_framed(&bytes).await
    }

    /// Receive the next `WireFrame` from the underlying TLS stream.
    pub async fn recv_frame(&self) -> Result<WireFrame> {
        let bytes = self.read_framed().await?;
        borderless_core::decode(&bytes).map_err(|e| Error::Codec(format!("decode: {e}")))
    }

    /// Cleanly close the connection.
    pub async fn close(&self) {
        let mut guard = self.inner.lock().await;
        let _ = guard.shutdown().await;
    }

    async fn write_framed(&self, payload: &[u8]) -> Result<()> {
        if payload.len() > MAX_FRAME {
            return Err(Error::Codec(format!(
                "frame too large: {} > {}",
                payload.len(),
                MAX_FRAME
            )));
        }
        let mut guard = self.inner.lock().await;
        let len = (payload.len() as u32).to_le_bytes();
        guard.write_all(&len).await?;
        guard.write_all(payload).await?;
        guard.flush().await?;
        Ok(())
    }

    async fn read_framed(&self) -> Result<Vec<u8>> {
        let mut guard = self.inner.lock().await;
        let mut len_buf = [0u8; 4];
        guard.read_exact(&mut len_buf).await?;
        let len = u32::from_le_bytes(len_buf) as usize;
        if len > MAX_FRAME {
            return Err(Error::Codec(format!("frame too large: {len}")));
        }
        let mut payload = vec![0u8; len];
        guard.read_exact(&mut payload).await?;
        Ok(payload)
    }
}

/// Application-layer handshake: exchange `SignedHello`s, verify against
/// the [`PeerStore`], optionally TOFU-insert new peers.
///
/// `is_initiator` selects who sends first; the connector side sends
/// before reading, the listener side reads before sending. This avoids
/// a deadlock where both sides block on read.
///
/// `exporter` is the TLS keying-material exporter computed from the
/// underlying connection BEFORE it was wrapped into the
/// [`TlsStream`] enum (the unified enum only exposes `CommonState`,
/// which doesn't have `export_keying_material`; the server-/client-
/// typed variants do).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handshake(
    mut tls: TlsStream<tokio::net::TcpStream>,
    exporter: [u8; 32],
    identity: &Identity,
    peers: &Arc<Mutex<PeerStore>>,
    name: &str,
    accept_new_peers: bool,
    expected_pubkey: Option<[u8; 32]>,
    is_initiator: bool,
) -> Result<Connection> {
    let mut binding = Vec::with_capacity(HELLO_BIND_LABEL.len() + exporter.len());
    binding.extend_from_slice(HELLO_BIND_LABEL);
    binding.extend_from_slice(&exporter);

    let signature = identity.sign(&binding);
    let our_hello = SignedHello {
        pubkey: identity.pubkey(),
        name: name.to_string(),
        max_protocol: PROTOCOL_V0,
        signature: signature.to_vec(),
    };
    let our_hello_bytes = borderless_core::encode(&our_hello)
        .map_err(|e| Error::Codec(format!("hello encode: {e}")))?;

    let peer_hello: SignedHello = if is_initiator {
        write_framed_raw(&mut tls, &our_hello_bytes).await?;
        read_hello(&mut tls).await?
    } else {
        let peer = read_hello(&mut tls).await?;
        write_framed_raw(&mut tls, &our_hello_bytes).await?;
        peer
    };

    if peer_hello.signature.len() != 64 {
        return Err(Error::Identity(format!(
            "bad signature length: {}",
            peer_hello.signature.len()
        )));
    }
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&peer_hello.signature);
    verify_signature(&peer_hello.pubkey, &binding, &sig)
        .map_err(|e| Error::Identity(format!("peer hello sig: {e}")))?;

    if let Some(expected) = expected_pubkey {
        if expected != peer_hello.pubkey {
            return Err(Error::Pairing(format!(
                "expected node {} but peer presented {}",
                hex::encode(expected),
                hex::encode(peer_hello.pubkey)
            )));
        }
    }

    let peer_node_id = NodeId::from_pubkey(&peer_hello.pubkey);
    {
        let mut store = peers.lock();
        if !store.is_known(&peer_hello.pubkey) {
            if !accept_new_peers {
                return Err(Error::Pairing(format!(
                    "unknown peer {} ({}); pair first",
                    peer_node_id, peer_hello.name
                )));
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            store.insert(peer_hello.pubkey, peer_hello.name.clone(), now)?;
        }
    }

    Ok(Connection {
        inner: Arc::new(AsyncMutex::new(tls)),
        peer_pubkey: peer_hello.pubkey,
        peer_name: peer_hello.name,
        peer_node_id,
    })
}

async fn read_hello(tls: &mut TlsStream<tokio::net::TcpStream>) -> Result<SignedHello> {
    let mut len_buf = [0u8; 4];
    tls.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > 16 * 1024 {
        return Err(Error::Codec(format!("hello too large: {len}")));
    }
    let mut payload = vec![0u8; len];
    tls.read_exact(&mut payload).await?;
    borderless_core::decode(&payload).map_err(|e| Error::Codec(format!("hello decode: {e}")))
}

async fn write_framed_raw(
    tls: &mut TlsStream<tokio::net::TcpStream>,
    payload: &[u8],
) -> Result<()> {
    let len = (payload.len() as u32).to_le_bytes();
    tls.write_all(&len).await?;
    tls.write_all(payload).await?;
    tls.flush().await?;
    Ok(())
}
