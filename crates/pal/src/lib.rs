//! Platform Abstraction Layer.
//!
//! Each desktop OS implements these three traits; the rest of the
//! borderless tree is platform-agnostic. Keep the trait surface small
//! and IO-explicit so it is easy to write fakes for tests.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

use async_trait::async_trait;
use borderless_core::{ClipboardSnapshot, InputEvent};
use thiserror::Error;
use tokio::sync::mpsc;

/// PAL-level error.
#[derive(Debug, Error)]
pub enum PalError {
    /// Operation is not supported on this platform / by this backend.
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
    /// Required permission missing (e.g. macOS Accessibility).
    #[error("permission required: {0}")]
    PermissionRequired(String),
    /// Underlying OS or library error.
    #[error("backend: {0}")]
    Backend(String),
    /// Generic IO error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// PAL result alias.
pub type PalResult<T> = std::result::Result<T, PalError>;

/// How aggressively to capture the input. The Active node grabs all
/// input; the Passive node lets local input pass through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureMode {
    /// Don't intercept anything (Passive).
    Off,
    /// Listen but don't suppress local delivery; used during boundary
    /// detection on the Active node before crossing.
    Listen,
    /// Fully grab: intercept and suppress local delivery while the
    /// cursor logically lives on a remote screen.
    Grab,
}

/// Sink for events captured by the platform backend.
///
/// Backends are async-friendly but not required to be `Send`-bound to
/// any specific runtime; mpsc is the integration point.
pub type EventSink = mpsc::UnboundedSender<InputEvent>;

/// Capture local input.
#[async_trait]
pub trait InputCapture: Send + 'static {
    /// Begin capturing. Events are delivered to `sink`.
    async fn start(&mut self, sink: EventSink) -> PalResult<()>;
    /// Stop capturing.
    async fn stop(&mut self) -> PalResult<()>;
    /// Update mode without restarting.
    async fn set_mode(&mut self, mode: CaptureMode) -> PalResult<()>;
}

/// Inject input into the local OS.
#[async_trait]
pub trait InputEmit: Send + 'static {
    /// Inject one event.
    async fn emit(&mut self, event: InputEvent) -> PalResult<()>;
}

/// Read & write the local clipboard, with change notifications.
#[async_trait]
pub trait Clipboard: Send + 'static {
    /// Read the current clipboard contents as a v0.1 text snapshot.
    /// Returns `None` if the clipboard isn't text-shaped (image, files, ...)
    /// for the v0.1 MVP that only handles text.
    async fn read(&mut self) -> PalResult<Option<String>>;

    /// Replace the clipboard with this snapshot. Implementations should
    /// pick the richest representation supported (v0.1 = text).
    async fn write(&mut self, snapshot: &ClipboardSnapshot) -> PalResult<()>;

    /// Subscribe to local clipboard changes. Each tick yields the new
    /// text content (the v0.1 simplification).
    fn watch(&mut self) -> mpsc::UnboundedReceiver<String>;
}
