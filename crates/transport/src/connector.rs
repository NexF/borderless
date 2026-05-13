//! Spoke-side TCP+TLS connector.

use crate::cert::client_config;
use crate::connection::{handshake, Connection, HELLO_BIND_LABEL};
use crate::identity::Identity;
use crate::peer_store::PeerStore;
use crate::{Error, Result};

use parking_lot::Mutex;
use rustls::pki_types::ServerName;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, TlsStream};

/// Spoke-side connector. Cheap to clone and reuse across reconnects.
#[derive(Clone)]
pub struct Connector {
    identity: Identity,
    peers: Arc<Mutex<PeerStore>>,
    connector: TlsConnector,
    /// Display name to send in our SignedHello.
    pub name: String,
}

impl Connector {
    /// Build a connector with an "accept any TLS cert, verify at the
    /// application layer" client config.
    pub fn new(identity: Identity, peers: Arc<Mutex<PeerStore>>, name: String) -> Result<Self> {
        let cfg = client_config()?;
        let connector = TlsConnector::from(cfg);
        Ok(Self {
            identity,
            peers,
            connector,
            name,
        })
    }

    /// Dial `addr`. If `expected_node_pubkey` is set, the handshake
    /// fails unless the peer presents that exact key.
    ///
    /// `accept_new_peer` mirrors the v0.1 `--pair` flag: with it set,
    /// the first successful Hello inserts the peer into the
    /// [`PeerStore`] (TOFU). Without it, the peer must already be
    /// known.
    pub async fn dial(
        &self,
        addr: SocketAddr,
        expected_node_pubkey: Option<[u8; 32]>,
        accept_new_peer: bool,
    ) -> Result<Connection> {
        let tcp = TcpStream::connect(addr).await?;
        let _ = tcp.set_nodelay(true);

        // The actual server name is irrelevant — our verifier accepts
        // any cert. We still need a syntactically valid SNI, so use a
        // fixed placeholder.
        let server_name = ServerName::try_from("borderless")
            .map_err(|e| Error::Tls(format!("server name: {e}")))?;
        let tls = self
            .connector
            .connect(server_name, tcp)
            .await
            .map_err(|e| Error::Tls(format!("connect tls: {e}")))?;

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
            &self.name,
            accept_new_peer,
            expected_node_pubkey,
            /*is_initiator*/ true,
        )
        .await
    }
}
