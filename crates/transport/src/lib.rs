//! QUIC-based transport for borderless.
//!
//! Layered on top of [`quinn`] + [`rustls`]:
//!
//! * Each node owns a long-term Ed25519 *identity key* persisted to disk.
//!   The [`NodeId`](borderless_core::NodeId) is BLAKE3-truncated from
//!   that public key.
//! * Each session uses a freshly generated ECDSA P-256 cert for TLS;
//!   the TLS layer is treated as anonymous (the verifier accepts any
//!   cert). Authentic identity is established by a signed `Hello`
//!   bound to the TLS keying-material exporter.
//! * Trust-on-first-use: paired peers are stored by Ed25519 public
//!   key fingerprint in `known_peers.toml`. Subsequent reconnects
//!   refuse mismatching fingerprints.
//!
//! See [`docs/architecture.md`](../../docs/architecture.md).

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod cert;
pub mod discovery;
pub mod endpoint;
pub mod identity;
pub mod peer_store;
pub mod sas;

pub use endpoint::{Connection, Endpoint, EndpointConfig};
pub use identity::Identity;
pub use peer_store::{KnownPeer, PeerStore};
pub use sas::{sas_digits, ShortAuthString};

use thiserror::Error;

/// Crate-level error.
#[derive(Debug, Error)]
pub enum Error {
    /// IO error from disk / sockets.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// QUIC connection problem.
    #[error("quic: {0}")]
    Connection(#[from] quinn::ConnectionError),
    /// QUIC dial problem.
    #[error("connect: {0}")]
    Connect(#[from] quinn::ConnectError),
    /// Stream write problem.
    #[error("write: {0}")]
    Write(#[from] quinn::WriteError),
    /// Stream read problem.
    #[error("read: {0}")]
    Read(#[from] quinn::ReadError),
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
    /// mDNS / discovery.
    #[error("discovery: {0}")]
    Discovery(String),
    /// Pairing protocol (e.g. SAS mismatch / unknown peer).
    #[error("pairing: {0}")]
    Pairing(String),
    /// Generic catch-all for context-rich errors.
    #[error("{0}")]
    Other(String),
}

/// Crate result alias.
pub type Result<T> = std::result::Result<T, Error>;
