//! Windows platform backend.
//!
//! v0.2 ships:
//! - Working text clipboard read/write/watch via `arboard`
//! - Real `InputCapture` via `WH_KEYBOARD_LL` + `WH_MOUSE_LL` hooks on
//!   a dedicated message-pump thread
//! - Real `InputEmit` via batched `SendInput` calls (scan-code based)
//!
//! On non-Windows targets the crate compiles to an empty stub so the
//! cargo workspace builds anywhere.

#![cfg_attr(not(target_os = "windows"), allow(dead_code, unused_imports))]

#[cfg(target_os = "windows")]
mod capture;
#[cfg(target_os = "windows")]
mod emit;
#[cfg(target_os = "windows")]
mod imp;
#[cfg(target_os = "windows")]
pub mod keymap;

#[cfg(target_os = "windows")]
pub use imp::{WindowsCapture, WindowsClipboard, WindowsEmit};

#[cfg(not(target_os = "windows"))]
/// Marker type for cross-platform `cargo check`.
pub struct WindowsStub;
