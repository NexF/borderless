//! TCP + TLS transport for borderless v0.2.
//!
//! Architecture:
//!
//! * Each node owns a long-term Ed25519 *identity key* persisted to disk.
//!   The [`NodeId`](borderless_core::NodeId) is BLAKE3-truncated from
//!   that public key.
//! * The Hub binds a TCP socket and terminates rustls TLS using a
//!   freshly generated ECDSA P-256 certificate. Spokes are clients;
//!   they accept any server certificate at the TLS layer.
//! * Real authentication runs at the application layer: both ends
//!   exchange a [`SignedHello`] containing an Ed25519 signature over
//!   the TLS exporter (`rustls::CommonState::export_keying_material`).
//!   This binds the long-term identity to the live TLS session.
//! * Trust-on-first-use: paired peers are stored by Ed25519 public
//!   key fingerprint in `known_peers.toml`. Subsequent reconnects
//!   refuse mismatching fingerprints.
//!
//! See [`docs/architecture.md`](../../docs/architecture.md).

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod cert;
pub mod connection;
pub mod connector;
pub mod identity;
pub mod listener;
pub mod peer_store;
pub mod sas;

pub use connection::{Connection, SignedHello, HELLO_BIND_LABEL};
pub use connector::Connector;
pub use identity::Identity;
pub use listener::{Listener, ListenerConfig};
pub use peer_store::{KnownPeer, PeerStore};
pub use sas::{sas_digits, ShortAuthString};

use thiserror::Error;

/// Crate-level error.
#[derive(Debug, Error)]
pub enum Error {
    /// IO error from disk / sockets.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// rustls problem.
    #[error("rustls: {0}")]
    Rustls(#[from] rustls::Error),
    /// rcgen problem.
    #[error("rcgen: {0}")]
    Rcgen(#[from] rcgen::Error),
    /// Codec / framing problem.
    #[error("codec: {0}")]
    Codec(String),
    /// Identity / signature verification.
    #[error("identity: {0}")]
    Identity(String),
    /// TOML config (peer store).
    #[error("toml: {0}")]
    Toml(String),
    /// Pairing protocol (e.g. SAS mismatch / unknown peer).
    #[error("pairing: {0}")]
    Pairing(String),
    /// TLS handshake / negotiation failure.
    #[error("tls: {0}")]
    Tls(String),
    /// Generic catch-all for context-rich errors.
    #[error("{0}")]
    Other(String),
}

/// Crate result alias.
pub type Result<T> = std::result::Result<T, Error>;
