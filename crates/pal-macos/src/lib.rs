//! macOS platform backend.
//!
//! v0.1 MVP scope: working text-clipboard via [`arboard`]; capture and
//! emit are stubs. The plan for v0.2:
//!
//! * `InputCapture`: `CGEventTap` at the session level, run on a
//!   dedicated thread that owns a `CFRunLoop`. **Requires the
//!   Accessibility + Input Monitoring permissions** (the user has to
//!   tick the box in System Settings → Privacy & Security; we surface
//!   this from `borderless doctor`).
//! * `InputEmit`: `CGEventPost(kCGHIDEventTap, ...)`.
//!
//! Compiles to a stub on non-macOS.

#![cfg_attr(not(target_os = "macos"), allow(dead_code, unused_imports))]

#[cfg(target_os = "macos")]
mod imp;

#[cfg(target_os = "macos")]
pub use imp::{MacosCapture, MacosClipboard, MacosEmit};

#[cfg(not(target_os = "macos"))]
/// Marker type for cross-platform `cargo check`.
pub struct MacosStub;
