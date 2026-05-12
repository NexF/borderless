//! High-level endpoint: a single QUIC socket that can both accept and
//! initiate connections, with per-connection identity verification.
//!
//! The wire shape on each connection:
//!
//! 1. Initiator opens a uni stream and sends a [`SignedHello`]; acceptor
//!    opens its own uni stream in reply with another [`SignedHello`].
//! 2. After both Hellos are validated, the connection is considered
//!    "authenticated": further bidi/uni streams carry length-prefixed
//!    [`WireFrame`](borderless_core::WireFrame)s and unreliable
//!    datagrams carry single [`InputEvent`](borderless_core::InputEvent)s.
//!
//! Each [`WireFrame`] is `u32-LE length || postcard payload`.

use crate::cert::{client_config, ephemeral_self_signed, server_config};
use crate::identity::{verify_signature, Identity};
use crate::peer_store::PeerStore;
use crate::{Error, Result};

use borderless_core::{ControlFrame, NodeId, ProtocolVersion, WireFrame, PROTOCOL_V0};
use parking_lot::Mutex;
use quinn::{
    crypto::rustls::{QuicClientConfig, QuicServerConfig},
    ClientConfig as QClientConfig, ServerConfig as QServerConfig,
};
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Domain-separation tag mixed into the Hello signature.
const HELLO_BIND_LABEL: &[u8] = b"borderless/hello/v0";

/// Endpoint configuration.
#[derive(Clone, Debug)]
pub struct EndpointConfig {
    /// UDP socket to bind. Use port 0 for an ephemeral port (mostly in tests).
    pub bind: SocketAddr,
    /// Human-readable name announced in `Hello`.
    pub name: String,
    /// If true, accept brand-new peers (Trust-On-First-Use). If false,
    /// only peers already in the [`PeerStore`] are accepted.
    pub allow_new_peers: bool,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)),
            name: hostname_or("borderless"),
            allow_new_peers: false,
        }
    }
}

fn hostname_or(default: &str) -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Application-layer Hello, signed by the long-term Ed25519 key.
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

/// Per-connection handle.
pub struct Connection {
    inner: quinn::Connection,
    /// Verified peer pubkey (32 bytes).
    pub peer_pubkey: [u8; 32],
    /// Verified peer name (from Hello).
    pub peer_name: String,
    /// Verified peer NodeId.
    pub peer_node_id: NodeId,
}

impl Connection {
    /// Send a `WireFrame` reliably on a fresh uni stream.
    pub async fn send_frame(&self, frame: &WireFrame) -> Result<()> {
        let bytes =
            borderless_core::encode(frame).map_err(|e| Error::Codec(format!("encode: {e}")))?;
        let mut stream = self.inner.open_uni().await?;
        let len = (bytes.len() as u32).to_le_bytes();
        stream.write_all(&len).await?;
        stream.write_all(&bytes).await?;
        stream.finish().map_err(|e| Error::Codec(e.to_string()))?;
        Ok(())
    }

    /// Receive the next `WireFrame` from any incoming uni stream.
    pub async fn recv_frame(&self) -> Result<WireFrame> {
        let mut stream = self.inner.accept_uni().await.map_err(Error::Connection)?;
        let mut len_buf = [0u8; 4];
        read_exact(&mut stream, &mut len_buf).await?;
        let len = u32::from_le_bytes(len_buf) as usize;
        if len > 64 * 1024 * 1024 {
            return Err(Error::Codec(format!("frame too large: {len}")));
        }
        let mut payload = vec![0u8; len];
        read_exact(&mut stream, &mut payload).await?;
        borderless_core::decode(&payload).map_err(|e| Error::Codec(format!("decode: {e}")))
    }

    /// Send a single `InputEvent` over an unreliable QUIC datagram.
    pub fn send_datagram(&self, event: &borderless_core::InputEvent) -> Result<()> {
        let bytes =
            borderless_core::encode(event).map_err(|e| Error::Codec(format!("encode: {e}")))?;
        self.inner
            .send_datagram(bytes.into())
            .map_err(|e| Error::Codec(format!("datagram: {e}")))
    }

    /// Wait for the next datagram and decode it as an `InputEvent`.
    pub async fn recv_datagram(&self) -> Result<borderless_core::InputEvent> {
        let bytes = self
            .inner
            .read_datagram()
            .await
            .map_err(Error::Connection)?;
        borderless_core::decode(&bytes).map_err(|e| Error::Codec(format!("datagram decode: {e}")))
    }

    /// Underlying quinn connection (escape hatch for tests / advanced use).
    pub fn quinn(&self) -> &quinn::Connection {
        &self.inner
    }

    /// Local socket address.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        // The connection itself doesn't expose local addr; callers use
        // `Endpoint::local_addr` instead.
        None
    }

    /// Close cleanly with a 0 application code.
    pub fn close(&self) {
        self.inner.close(0u32.into(), b"bye");
    }
}

async fn read_exact(stream: &mut quinn::RecvStream, buf: &mut [u8]) -> Result<()> {
    let mut filled = 0;
    while filled < buf.len() {
        match stream.read(&mut buf[filled..]).await {
            Ok(Some(n)) => filled += n,
            Ok(None) => return Err(Error::Codec("unexpected eof".into())),
            Err(e) => return Err(Error::Codec(format!("read: {e}"))),
        }
    }
    Ok(())
}

/// Top-level transport endpoint.
pub struct Endpoint {
    inner: quinn::Endpoint,
    identity: Identity,
    peers: Arc<Mutex<PeerStore>>,
    config: EndpointConfig,
    /// Used to surface unknown-but-allowed peers to a pairing UI; in
    /// pairing mode the CLI subscribes here.
    new_peer_tx: Option<mpsc::UnboundedSender<NewPeerEvent>>,
}

/// Surfaced when a peer connects whose pubkey is not yet in the store.
#[derive(Clone, Debug)]
pub struct NewPeerEvent {
    /// Their pubkey.
    pub pubkey: [u8; 32],
    /// Their display name.
    pub name: String,
    /// Their NodeId.
    pub node_id: NodeId,
}

impl Endpoint {
    /// Bind a UDP socket and return an [`Endpoint`].
    pub fn bind(
        identity: Identity,
        peers: Arc<Mutex<PeerStore>>,
        config: EndpointConfig,
    ) -> Result<Self> {
        let (cert, key) = ephemeral_self_signed()?;
        let server_crypto = server_config(cert, key)?;
        let server_quic_crypto = QuicServerConfig::try_from(server_crypto.as_ref().clone())
            .map_err(|e| Error::Other(format!("quic server cfg: {e}")))?;
        let mut server_conf = QServerConfig::with_crypto(Arc::new(server_quic_crypto));

        let mut transport = quinn::TransportConfig::default();
        transport
            .keep_alive_interval(Some(Duration::from_secs(5)))
            .max_concurrent_uni_streams(quinn::VarInt::from_u32(256));
        server_conf.transport_config(Arc::new(transport));

        let endpoint = quinn::Endpoint::server(server_conf, config.bind)?;

        Ok(Self {
            inner: endpoint,
            identity,
            peers,
            config,
            new_peer_tx: None,
        })
    }

    /// Subscribe to "new peer wants to pair" events. Only one
    /// subscriber is supported; later calls overwrite.
    pub fn subscribe_new_peers(&mut self) -> mpsc::UnboundedReceiver<NewPeerEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.new_peer_tx = Some(tx);
        rx
    }

    /// Local socket address.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.inner.local_addr()?)
    }

    /// Connect to a peer.
    pub async fn connect(&self, addr: SocketAddr) -> Result<Connection> {
        let crypto = client_config()?;
        let qcrypto = QuicClientConfig::try_from(crypto.as_ref().clone())
            .map_err(|e| Error::Other(format!("quic client cfg: {e}")))?;
        let mut client_cfg = QClientConfig::new(Arc::new(qcrypto));
        let mut transport = quinn::TransportConfig::default();
        transport.keep_alive_interval(Some(Duration::from_secs(5)));
        transport.max_concurrent_uni_streams(quinn::VarInt::from_u32(256));
        client_cfg.transport_config(Arc::new(transport));

        let connecting = self
            .inner
            .connect_with(client_cfg, addr, "borderless")
            .map_err(Error::Connect)?;
        let conn = connecting.await.map_err(Error::Connection)?;
        self.handshake(conn, /*is_initiator*/ true).await
    }

    /// Wait for the next inbound connection.
    pub async fn accept(&self) -> Result<Connection> {
        let incoming = self
            .inner
            .accept()
            .await
            .ok_or_else(|| Error::Other("endpoint closed".into()))?;
        let conn = incoming.await.map_err(Error::Connection)?;
        self.handshake(conn, /*is_initiator*/ false).await
    }

    /// Close the endpoint and wait for in-flight connections.
    pub async fn close(&self) {
        self.inner.close(0u32.into(), b"bye");
        self.inner.wait_idle().await;
    }

    async fn handshake(&self, conn: quinn::Connection, is_initiator: bool) -> Result<Connection> {
        // Channel-binding: hash the TLS exporter keying material for the
        // peer to sign over.
        let mut exporter = [0u8; 32];
        conn.export_keying_material(&mut exporter, b"borderless/hello/v0", b"")
            .map_err(|e| Error::Other(format!("exporter: {e:?}")))?;
        let mut binding = Vec::with_capacity(HELLO_BIND_LABEL.len() + exporter.len());
        binding.extend_from_slice(HELLO_BIND_LABEL);
        binding.extend_from_slice(&exporter);

        let signature = self.identity.sign(&binding);
        let our_hello = SignedHello {
            pubkey: self.identity.pubkey(),
            name: self.config.name.clone(),
            max_protocol: PROTOCOL_V0,
            signature: signature.to_vec(),
        };
        let our_hello_bytes = borderless_core::encode(&our_hello)
            .map_err(|e| Error::Codec(format!("hello encode: {e}")))?;

        // Initiator sends first. Acceptor reads first. This avoids a
        // deadlock where both sides try to accept_uni without anyone
        // having opened.
        let peer_hello: SignedHello = if is_initiator {
            send_uni(&conn, &our_hello_bytes).await?;
            recv_uni(&conn).await?
        } else {
            let peer = recv_uni::<SignedHello>(&conn).await?;
            send_uni(&conn, &our_hello_bytes).await?;
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

        let peer_node_id = NodeId::from_pubkey(&peer_hello.pubkey);
        {
            let mut peers = self.peers.lock();
            if !peers.is_known(&peer_hello.pubkey) {
                if !self.config.allow_new_peers {
                    return Err(Error::Pairing(format!(
                        "unknown peer {} ({}); pair first",
                        peer_node_id, peer_hello.name
                    )));
                }
                if let Some(tx) = &self.new_peer_tx {
                    let _ = tx.send(NewPeerEvent {
                        pubkey: peer_hello.pubkey,
                        name: peer_hello.name.clone(),
                        node_id: peer_node_id,
                    });
                }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                peers.insert(peer_hello.pubkey, peer_hello.name.clone(), now)?;
            } else {
                // Already paired: nothing to do, but the lookup
                // confirms the fingerprint matches.
            }
        }

        Ok(Connection {
            inner: conn,
            peer_pubkey: peer_hello.pubkey,
            peer_name: peer_hello.name,
            peer_node_id,
        })
    }

    /// Build a hello-style control frame for use after handshake.
    pub fn make_local_hello_frame(&self) -> WireFrame {
        WireFrame::Control(ControlFrame::Hello {
            node_id: self.identity.node_id(),
            name: self.config.name.clone(),
            max_protocol: PROTOCOL_V0,
        })
    }
}

async fn send_uni(conn: &quinn::Connection, payload: &[u8]) -> Result<()> {
    let mut stream = conn.open_uni().await?;
    let len = (payload.len() as u32).to_le_bytes();
    stream.write_all(&len).await?;
    stream.write_all(payload).await?;
    stream.finish().map_err(|e| Error::Codec(e.to_string()))?;
    Ok(())
}

async fn recv_uni<T: serde::de::DeserializeOwned>(conn: &quinn::Connection) -> Result<T> {
    let mut stream = conn.accept_uni().await.map_err(Error::Connection)?;
    let mut len_buf = [0u8; 4];
    read_exact(&mut stream, &mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > 16 * 1024 {
        return Err(Error::Codec(format!("hello too large: {len}")));
    }
    let mut payload = vec![0u8; len];
    read_exact(&mut stream, &mut payload).await?;
    borderless_core::decode(&payload).map_err(|e| Error::Codec(format!("decode: {e}")))
}
