//! Windows platform backend.
//!
//! v0.1 MVP scope: working text-clipboard via [`arboard`]; capture and
//! emit are stubs marked `Unsupported`. The plan for v0.2 is:
//!
//! * `InputCapture`: low-level keyboard + mouse hooks via
//!   `SetWindowsHookEx(WH_MOUSE_LL / WH_KEYBOARD_LL)`. Hook callback
//!   pumps a Windows message loop on a dedicated thread, forwarding
//!   to the [`borderless_pal::EventSink`].
//! * `InputEmit`: `SendInput` with a batched `INPUT` array.
//!
//! On non-Windows targets the crate compiles to an empty stub so the
//! cargo workspace builds anywhere.

#![cfg_attr(not(target_os = "windows"), allow(dead_code, unused_imports))]

#[cfg(target_os = "windows")]
mod imp;

#[cfg(target_os = "windows")]
pub use imp::{WindowsCapture, WindowsClipboard, WindowsEmit};

#[cfg(not(target_os = "windows"))]
/// Marker type for cross-platform `cargo check`.
pub struct WindowsStub;
