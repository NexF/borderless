//! X11 platform backend.
//!
//! v0.2 ships:
//! - Working text clipboard read/write/watch via `arboard`
//! - Real `InputCapture` via XInput2 raw events on a dedicated thread
//! - Real `InputEmit` via the XTest extension (HID → keysym → keycode)
//!
//! Compiles to an essentially empty crate on non-Linux targets so
//! `cargo build --workspace` works cross-platform.

#![cfg_attr(not(target_os = "linux"), allow(dead_code, unused_imports))]

#[cfg(target_os = "linux")]
mod capture;
#[cfg(target_os = "linux")]
mod emit;
#[cfg(target_os = "linux")]
mod imp;
#[cfg(target_os = "linux")]
pub mod keymap;

#[cfg(target_os = "linux")]
pub use imp::{X11Capture, X11Clipboard, X11Emit};

#[cfg(not(target_os = "linux"))]
/// Marker type that satisfies the `borderless-cli` link on non-Linux
/// hosts so the workspace can be built once for all platforms.
pub struct X11Stub;
