//! Hub and Spoke runtimes.
//!
//! v0.2 splits the daemon into two roles:
//!
//! * [`server::ServerRuntime`] — Hub. Binds a TCP+TLS listener,
//!   accepts spokes, fans out clipboard updates, drives input
//!   capture (in v0.2 still stubbed at the PAL layer; the runtime
//!   wiring is in place).
//! * [`client::ClientRuntime`] — Spoke. Dials the configured Hub,
//!   reconnects with exponential backoff, applies clipboard updates
//!   locally, and forwards local clipboard changes back to the Hub.

pub mod client;
pub mod common;
pub mod server;

pub use client::ClientRuntime;
pub use server::ServerRuntime;
