//! Versioned clipboard snapshots and the lazy-payload mechanism.

use crate::node::NodeId;
use serde::{Deserialize, Serialize};

/// Logical clock for clipboard updates.
///
/// We use a Lamport-style counter rather than wall-clock time so that
/// node clock skew can't cause an "older" update to overwrite a newer
/// one. Each node increments this on every local clipboard change and
/// MUST advance to `max(local, remote) + 1` when accepting a remote
/// snapshot.
pub type ClipVersion = u64;

/// A self-contained clipboard update carrying any number of MIME-shaped
/// items. Multiple items in one snapshot let the receiver pick the
/// richest representation it can render.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardSnapshot {
    /// Lamport version. Strictly monotonic per-origin.
    pub version: ClipVersion,
    /// Originating node. Used to suppress self-echo loops.
    pub origin: NodeId,
    /// Wall-clock timestamp at the source (informational only).
    pub created_unix_ms: u64,
    /// Available representations of this snapshot.
    pub items: Vec<ClipItem>,
}

/// One representation of a clipboard payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipItem {
    /// Plain UTF-8 text. The v0.1 MVP exclusively traffics in this
    /// variant; the others are placeholders for v0.2 / v0.3.
    Text(String),
    /// HTML, with a plain-text fallback for receivers that can't
    /// render HTML.
    Html {
        /// Raw HTML markup.
        html: String,
        /// Plain-text fallback derived from the HTML.
        plain_fallback: String,
    },
    /// Bitmap image.
    Image {
        /// Image format.
        format: ImageFormat,
        /// BLAKE3 hash of `data` (after resolving lazy payloads).
        hash: [u8; 32],
        /// The image bytes (inline) or a fetch handle (on-demand).
        data: LazyPayload,
    },
    /// One or more files / URIs.
    Files(Vec<FileRef>),
}

/// Recognized clipboard image formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImageFormat {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
    /// Raw 32-bit RGBA bitmap.
    Rgba8,
}

/// A reference to a file in a clipboard payload. v0.3 will lift this
/// from "path" to "lazy stream"; in v0.1 we only define the shape so
/// upstream code can match exhaustively today.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileRef {
    /// A path local to the originating node. Receivers MUST NOT use
    /// this verbatim (the path is meaningless on their filesystem) and
    /// instead request the file via lazy fetch.
    OriginPath(String),
    /// Inline bytes for tiny files.
    Inline {
        /// File name without directory.
        name: String,
        /// Raw bytes.
        data: Vec<u8>,
    },
    /// Out-of-line: receiver opens a fetch stream by hash.
    OnDemand {
        /// File name.
        name: String,
        /// Total size in bytes.
        size: u64,
        /// BLAKE3 hash, the fetch key.
        hash: [u8; 32],
    },
}

/// Either inline bytes or an out-of-line fetch handle, chosen by size.
///
/// The clipboard layer uses a 256 KiB threshold by default: anything
/// larger ships as `OnDemand` so a 500 MB clipboard copy doesn't
/// freeze the mesh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LazyPayload {
    /// Bytes ride along inside the snapshot.
    Inline(Vec<u8>),
    /// Receiver opens a per-hash fetch stream on paste.
    OnDemand {
        /// BLAKE3 hash.
        hash: [u8; 32],
        /// Total size in bytes.
        size: u64,
    },
}
