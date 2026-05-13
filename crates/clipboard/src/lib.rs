//! Clipboard sync engine: versioning, anti-loop, and a small history.
//!
//! This crate sits between the [`borderless_pal::Clipboard`] backend
//! and the network. It implements:
//!
//! 1. **Lamport versioning.** A monotonic counter advanced on every
//!    locally-observed change AND on every accepted remote snapshot.
//!    Snapshots strictly older than what we've already applied are
//!    dropped, eliminating the A → B → A echo loop.
//! 2. **Origin filtering.** Snapshots originating from our own
//!    [`NodeId`] are ignored on receive.
//! 3. **Bounded history.** A ring buffer keeps the last N snapshots so
//!    the CLI can show `clip history`.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub mod lazy;

pub use lazy::{produce_image, serve_fetch, LazyStore, DEFAULT_TOTAL_CAP, INLINE_THRESHOLD};

use borderless_core::{ClipItem, ClipboardSnapshot, NodeId};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::trace;

/// Crate error.
#[derive(Debug, Error)]
pub enum Error {
    /// Snapshot didn't move the clock forward.
    #[error("stale snapshot: incoming version {incoming} <= local {local}")]
    Stale {
        /// Incoming version.
        incoming: u64,
        /// Local current version.
        local: u64,
    },
    /// Snapshot loops back to ourselves.
    #[error("self-originated snapshot rejected")]
    SelfOrigin,
}

/// Decision on what to do after [`Engine::observe_remote`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Apply the snapshot to the local OS clipboard.
    Apply(ClipboardSnapshot),
    /// Ignore (echo, stale, self-origin).
    Ignore,
}

/// In-process clipboard sync engine.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    self_id: NodeId,
    local_version: u64,
    history: VecDeque<ClipboardSnapshot>,
    history_limit: usize,
}

impl Engine {
    /// New engine.
    pub fn new(self_id: NodeId, history_limit: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                self_id,
                local_version: 0,
                history: VecDeque::with_capacity(history_limit.max(1)),
                history_limit: history_limit.max(1),
            })),
        }
    }

    /// Build a snapshot from a string the local OS reported. Bumps the
    /// version. Returns the snapshot to broadcast.
    pub fn produce_text(&self, text: String) -> ClipboardSnapshot {
        let mut g = self.inner.lock();
        g.local_version += 1;
        let snap = ClipboardSnapshot {
            version: g.local_version,
            origin: g.self_id,
            created_unix_ms: now_ms(),
            items: vec![ClipItem::Text(text)],
        };
        g.push_history(snap.clone());
        snap
    }

    /// Build an image snapshot, using `store` to host the bytes if
    /// they exceed the lazy threshold.
    pub fn produce_image_snapshot(
        &self,
        store: &LazyStore,
        bytes: Vec<u8>,
        format: borderless_core::ImageFormat,
    ) -> ClipboardSnapshot {
        let item = produce_image(store, bytes, format);
        let mut g = self.inner.lock();
        g.local_version += 1;
        let snap = ClipboardSnapshot {
            version: g.local_version,
            origin: g.self_id,
            created_unix_ms: now_ms(),
            items: vec![item],
        };
        g.push_history(snap.clone());
        snap
    }

    /// Process an inbound snapshot. The caller should `Apply` the
    /// returned snapshot to the local OS clipboard and broadcast it
    /// further if relaying.
    pub fn observe_remote(&self, snap: ClipboardSnapshot) -> Decision {
        let mut g = self.inner.lock();
        if snap.origin == g.self_id {
            trace!(version = snap.version, "ignored self-origin snapshot");
            return Decision::Ignore;
        }
        if snap.version <= g.local_version {
            trace!(
                incoming = snap.version,
                local = g.local_version,
                "ignored stale snapshot"
            );
            return Decision::Ignore;
        }
        // Lamport advance: max(local, remote) — but we only got here
        // because remote is strictly greater.
        g.local_version = snap.version;
        g.push_history(snap.clone());
        Decision::Apply(snap)
    }

    /// Snapshot of the history (newest last).
    pub fn history(&self) -> Vec<ClipboardSnapshot> {
        self.inner.lock().history.iter().cloned().collect()
    }

    /// Current Lamport version.
    pub fn version(&self) -> u64 {
        self.inner.lock().local_version
    }
}

impl Inner {
    fn push_history(&mut self, snap: ClipboardSnapshot) {
        if self.history.len() == self.history_limit {
            self.history.pop_front();
        }
        self.history.push_back(snap);
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(b: u8) -> NodeId {
        NodeId([b; 16])
    }

    #[test]
    fn produce_advances_version_and_history() {
        let e = Engine::new(n(1), 8);
        let s1 = e.produce_text("a".into());
        let s2 = e.produce_text("b".into());
        assert_eq!(s1.version, 1);
        assert_eq!(s2.version, 2);
        let h = e.history();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].items, vec![ClipItem::Text("a".into())]);
        assert_eq!(h[1].items, vec![ClipItem::Text("b".into())]);
    }

    #[test]
    fn ignores_self_origin_snapshots() {
        let e = Engine::new(n(1), 8);
        let s = e.produce_text("a".into());
        assert_eq!(e.observe_remote(s), Decision::Ignore);
    }

    #[test]
    fn ignores_stale_versions() {
        let e = Engine::new(n(1), 8);
        let _s = e.produce_text("a".into());
        let _s = e.produce_text("b".into());
        let stale = ClipboardSnapshot {
            version: 1,
            origin: n(2),
            created_unix_ms: 0,
            items: vec![ClipItem::Text("x".into())],
        };
        assert_eq!(e.observe_remote(stale), Decision::Ignore);
    }

    #[test]
    fn applies_newer_remote_and_advances_clock() {
        let e = Engine::new(n(1), 8);
        let _ = e.produce_text("a".into());
        let remote = ClipboardSnapshot {
            version: 5,
            origin: n(2),
            created_unix_ms: 0,
            items: vec![ClipItem::Text("z".into())],
        };
        match e.observe_remote(remote.clone()) {
            Decision::Apply(s) => assert_eq!(s, remote),
            _ => panic!("expected apply"),
        }
        assert_eq!(e.version(), 5);
        // Producing locally now advances past the remote version.
        let next = e.produce_text("local".into());
        assert_eq!(next.version, 6);
    }

    #[test]
    fn three_node_chain_does_not_loop_back() {
        // Topology: A -> B -> C -> A. A produces; B forwards to C;
        // C forwards back to A. A must drop the echo of its own
        // snapshot, and B/C must each reject the second arrival as
        // stale.
        let a = Engine::new(n(1), 8);
        let b = Engine::new(n(2), 8);
        let c = Engine::new(n(3), 8);

        let snap = a.produce_text("hi".into());

        // A -> B (apply)
        match b.observe_remote(snap.clone()) {
            Decision::Apply(_) => {}
            _ => panic!("B should apply"),
        }
        // B -> C (apply)
        match c.observe_remote(snap.clone()) {
            Decision::Apply(_) => {}
            _ => panic!("C should apply"),
        }
        // C -> A: self-origin, must drop.
        assert_eq!(a.observe_remote(snap.clone()), Decision::Ignore);
        // A -> B again with the same snapshot: stale (version equal).
        assert_eq!(b.observe_remote(snap.clone()), Decision::Ignore);
        // C -> B: stale.
        assert_eq!(b.observe_remote(snap), Decision::Ignore);
    }

    #[test]
    fn after_remote_apply_local_produce_advances_past_remote() {
        // If we accept a remote at v=10, our next local produce must
        // be v=11, not v=1, so the network can't see two snapshots
        // sharing a clock value.
        let a = Engine::new(n(1), 8);
        let remote = ClipboardSnapshot {
            version: 10,
            origin: n(2),
            created_unix_ms: 0,
            items: vec![ClipItem::Text("remote".into())],
        };
        match a.observe_remote(remote) {
            Decision::Apply(_) => {}
            _ => panic!(),
        }
        let next = a.produce_text("local".into());
        assert_eq!(next.version, 11);
    }

    #[test]
    fn history_is_bounded() {
        let e = Engine::new(n(1), 3);
        for i in 0..10 {
            e.produce_text(format!("v{i}"));
        }
        assert_eq!(e.history().len(), 3);
    }
}
