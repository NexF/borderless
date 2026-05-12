//! Virtual screen layout, edge crossing, and modifier-state tracking.
//!
//! This crate is a *pure-logic* library: no IO, no platform calls. It
//! takes raw input events from the local PAL plus the published
//! topology, and decides which (if any) remote node should receive
//! synthesized [`InputEvent`]s.
//!
//! ## Layout
//!
//! Each node owns a logical rectangular screen. Edges of two screens
//! can be glued together: when the cursor walks past the boundary, we
//! emit an `InputEvent::Leave` toward the source and an
//! `InputEvent::Enter` toward the destination. The destination
//! receives the modifier-key bitmap so a held `Shift` doesn't desync.
//!
//! v0.1 only models 2-3 nodes in a single horizontal row. v0.2 will
//! lift this to arbitrary 2D adjacency.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod layout;
pub mod modifier;
pub mod router;

pub use layout::{Edge, Layout, Screen};
pub use modifier::ModifierState;
pub use router::{Routed, Router};
