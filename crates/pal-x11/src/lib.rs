//! X11 platform backend.
//!
//! v0.1 MVP scope:
//! - Working text-clipboard read/write/watch via [`arboard`]
//! - Stub `InputCapture` / `InputEmit` that wire up the trait surface
//!   but leave the actual XInput2 / XTest plumbing as a `TODO!()`
//!   marker. This is intentional: the cross-cutting concerns
//!   (transport, routing, clipboard sync) are valuable on their own
//!   and can be exercised in `cargo test` today on a Linux box without
//!   an X server. The XInput2 capture loop will land in v0.2.
//!
//! Compiles to an essentially empty crate on non-Linux targets so
//! `cargo build --workspace` works cross-platform.

#![cfg_attr(not(target_os = "linux"), allow(dead_code, unused_imports))]

#[cfg(target_os = "linux")]
mod imp;

#[cfg(target_os = "linux")]
pub use imp::{X11Capture, X11Clipboard, X11Emit};

#[cfg(not(target_os = "linux"))]
/// Marker type that satisfies the `borderless-cli` link on non-Linux
/// hosts so the workspace can be built once for all platforms.
pub struct X11Stub;
