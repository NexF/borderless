//! Pure protocol types for borderless.
//!
//! This crate has no IO and no platform-specific code so that the wire
//! format is trivially testable and future bindings (e.g. for a control
//! GUI or a future mobile peer) can depend on it cheaply.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod clipboard;
pub mod hid;
pub mod input;
pub mod node;
pub mod wire;

pub use clipboard::{ClipItem, ClipboardSnapshot, FileRef, ImageFormat, LazyPayload};
pub use hid::{HidUsage, ModifierMask};
pub use input::{Button, InputEvent};
pub use node::{NodeId, ProtocolVersion, PROTOCOL_V0};
pub use wire::{ControlFrame, WireFrame};

/// Errors surfaced from this crate (mostly serialization).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to encode a wire frame.
    #[error("postcard encode: {0}")]
    Encode(#[source] postcard::Error),
    /// Failed to decode a wire frame.
    #[error("postcard decode: {0}")]
    Decode(#[source] postcard::Error),
}

/// Convenience [`Result`] alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Encode a serializable value with `postcard`.
pub fn encode<T: serde::Serialize>(value: &T) -> Result<Vec<u8>> {
    postcard::to_allocvec(value).map_err(Error::Encode)
}

/// Decode a `postcard`-encoded value.
pub fn decode<'de, T: serde::Deserialize<'de>>(bytes: &'de [u8]) -> Result<T> {
    postcard::from_bytes(bytes).map_err(Error::Decode)
}
