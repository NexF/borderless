//! macOS platform backend.
//!
//! v0.2 ships:
//! - Working text clipboard read/write/watch via `arboard`
//! - Real `InputEmit` via `CGEventCreateKeyboardEvent` /
//!   `CGEventCreateMouseEvent` + `CGEventPost`
//! - `MacosCapture::start` checks `AXIsProcessTrusted` and refuses to
//!   start without Accessibility permission. The full CGEventTap +
//!   CFRunLoop scaffolding lands in v0.3 (it requires a richer
//!   permissions UX than v0.2 ships).
//!
//! Compiles to a stub on non-macOS.

#![cfg_attr(not(target_os = "macos"), allow(dead_code, unused_imports))]

#[cfg(target_os = "macos")]
mod capture;
#[cfg(target_os = "macos")]
mod emit;
#[cfg(target_os = "macos")]
mod imp;
#[cfg(target_os = "macos")]
pub mod keymap;

#[cfg(target_os = "macos")]
pub use imp::{MacosCapture, MacosClipboard, MacosEmit};

#[cfg(not(target_os = "macos"))]
/// Marker type for cross-platform `cargo check`.
pub struct MacosStub;
