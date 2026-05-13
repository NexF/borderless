//! Hub-side TCP+TLS listener.
//!
//! The Hub binds a TCP socket and terminates rustls TLS with an
//! ephemeral self-signed certificate. Every accepted connection runs
//! through the application-layer [`handshake`](crate::connection::handshake)
//! before the [`Connection`] is surfaced to the caller.

use crate::cert::{ephemeral_self_signed, server_config};
use crate::connection::{handshake, Connection, HELLO_BIND_LABEL};
use crate::identity::Identity;
use crate::peer_store::PeerStore;
use crate::{Error, Result};

use parking_lot::Mutex;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::{TlsAcceptor, TlsStream};

/// Listener configuration.
#[derive(Clone, Debug)]
pub struct ListenerConfig {
    /// TCP socket to bind.
    pub bind: SocketAddr,
    /// Local display name announced in the SignedHello.
    pub name: String,
    /// If true, accept brand-new spokes (Trust-On-First-Use). If false,
    /// only spokes already in the [`PeerStore`] are accepted.
    pub accept_new_peers: bool,
}

/// Hub-side listener.
pub struct Listener {
    listener: TcpListener,
    acceptor: TlsAcceptor,
    identity: Identity,
    peers: Arc<Mutex<PeerStore>>,
    config: ListenerConfig,
}

impl Listener {
    /// Bind a TCP socket and prepare a TLS acceptor.
    pub async fn bind(
        identity: Identity,
        peers: Arc<Mutex<PeerStore>>,
        config: ListenerConfig,
    ) -> Result<Self> {
        let (cert, key) = ephemeral_self_signed()?;
        let server_crypto = server_config(cert, key)?;
        let acceptor = TlsAcceptor::from(server_crypto);
        let listener = TcpListener::bind(config.bind).await?;
        Ok(Self {
            listener,
            acceptor,
            identity,
            peers,
            config,
        })
    }

    /// Local socket address.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    /// Wait for and authenticate the next inbound connection.
    pub async fn accept(&self) -> Result<Connection> {
        let (tcp, peer_addr) = self.listener.accept().await?;
        // Disable Nagle: input events are small and want to flush
        // immediately; clipboard frames batch fine over a few packets.
        let _ = tcp.set_nodelay(true);
        tracing::debug!(%peer_addr, "tcp accepted, starting tls");

        let tls = self
            .acceptor
            .accept(tcp)
            .await
            .map_err(|e| Error::Tls(format!("accept tls from {peer_addr}: {e}")))?;

        // Pull the TLS exporter while we still have access to the
        // typed `ServerConnection` (the unified `TlsStream` enum
        // exposes only `CommonState`, which lacks the method).
        let mut exporter = [0u8; 32];
        {
            let (_, conn) = tls.get_ref();
            conn.export_keying_material(&mut exporter, HELLO_BIND_LABEL, None)
                .map_err(|e| Error::Tls(format!("exporter: {e}")))?;
        }
        let tls = TlsStream::from(tls);
        handshake(
            tls,
            exporter,
            &self.identity,
            &self.peers,
            &self.config.name,
            self.config.accept_new_peers,
            None,
            /*is_initiator*/ false,
        )
        .await
    }
}
