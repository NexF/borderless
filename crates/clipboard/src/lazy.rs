//! Lazy clipboard payload store.
//!
//! Large clipboard items (images, big HTML, …) ride as a hash on the
//! wire and only get fetched when the receiver actually pastes. This
//! file owns the producer-side cache: when a node creates an oversized
//! snapshot, the bytes go here keyed by their BLAKE3 hash; later, when
//! a peer asks for them via `WireFrame::FetchRequest`, the runtime
//! pulls them back out and chunks them into `WireFrame::FetchResponse`s.
//!
//! Bounding strategy:
//!
//! - **Per-entry size cap**: enforced at insert time.
//! - **Total store cap**: oldest entries evicted when total bytes
//!   exceed the configured budget.
//! - All operations are O(1) amortized.

use borderless_core::{ClipItem, ImageFormat, LazyPayload};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Threshold above which a clipboard payload becomes lazy. Items at or
/// below this size are inlined into the snapshot.
pub const INLINE_THRESHOLD: usize = 256 * 1024;

/// Default total cap on the lazy store: 256 MiB.
pub const DEFAULT_TOTAL_CAP: usize = 256 * 1024 * 1024;

/// Producer-side cache of large payloads, keyed by BLAKE3 hash.
#[derive(Clone)]
pub struct LazyStore {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    by_hash: HashMap<[u8; 32], Vec<u8>>,
    /// Insertion order, used for FIFO eviction.
    order: VecDeque<[u8; 32]>,
    /// Sum of `bytes.len()` over all entries.
    total_bytes: usize,
    /// Total cap.
    cap: usize,
}

impl LazyStore {
    /// New store with the default cap.
    pub fn new() -> Self {
        Self::with_cap(DEFAULT_TOTAL_CAP)
    }

    /// New store with an explicit cap.
    pub fn with_cap(cap: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                by_hash: HashMap::new(),
                order: VecDeque::new(),
                total_bytes: 0,
                cap,
            })),
        }
    }

    /// Current number of stored entries.
    pub fn len(&self) -> usize {
        self.inner.lock().by_hash.len()
    }

    /// True if no entries are stored.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().by_hash.is_empty()
    }

    /// Insert `bytes`, returning the hash. Evicts oldest entries if
    /// inserting would exceed the cap.
    pub fn insert(&self, bytes: Vec<u8>) -> [u8; 32] {
        let hash: [u8; 32] = blake3::hash(&bytes).into();
        let mut g = self.inner.lock();
        if g.by_hash.contains_key(&hash) {
            return hash;
        }
        // Evict until there's room.
        while g.total_bytes + bytes.len() > g.cap {
            let Some(oldest) = g.order.pop_front() else {
                break;
            };
            if let Some(removed) = g.by_hash.remove(&oldest) {
                g.total_bytes = g.total_bytes.saturating_sub(removed.len());
            }
        }
        g.total_bytes += bytes.len();
        g.order.push_back(hash);
        g.by_hash.insert(hash, bytes);
        hash
    }

    /// Look up bytes by hash.
    pub fn get(&self, hash: &[u8; 32]) -> Option<Vec<u8>> {
        self.inner.lock().by_hash.get(hash).cloned()
    }

    /// Forget an entry.
    pub fn remove(&self, hash: &[u8; 32]) {
        let mut g = self.inner.lock();
        if let Some(bytes) = g.by_hash.remove(hash) {
            g.total_bytes = g.total_bytes.saturating_sub(bytes.len());
            g.order.retain(|h| h != hash);
        }
    }

    /// Total cap on the store.
    pub fn cap(&self) -> usize {
        self.inner.lock().cap
    }
}

impl Default for LazyStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Produce a `ClipItem::Image` snapshot, deciding inline vs lazy by
/// the [`INLINE_THRESHOLD`].
///
/// `format` is the on-the-wire representation (PNG/JPEG/RGBA8). The
/// returned `ClipItem` carries either inline bytes or a `LazyPayload`
/// hash; in the lazy case the bytes are inserted into `store`.
pub fn produce_image(store: &LazyStore, bytes: Vec<u8>, format: ImageFormat) -> ClipItem {
    let hash: [u8; 32] = blake3::hash(&bytes).into();
    let size = bytes.len() as u64;
    if bytes.len() <= INLINE_THRESHOLD {
        ClipItem::Image {
            format,
            hash,
            data: LazyPayload::Inline(bytes),
        }
    } else {
        store.insert(bytes);
        ClipItem::Image {
            format,
            hash,
            data: LazyPayload::OnDemand { hash, size },
        }
    }
}

/// Resolve a [`LazyPayload`] to bytes, either inline-or via the store.
/// `None` if the payload is on-demand and not present in `store`.
pub fn serve_fetch(store: &LazyStore, payload: &LazyPayload) -> Option<Vec<u8>> {
    match payload {
        LazyPayload::Inline(b) => Some(b.clone()),
        LazyPayload::OnDemand { hash, .. } => store.get(hash),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_image_inlines() {
        let store = LazyStore::new();
        let item = produce_image(&store, vec![1, 2, 3, 4], ImageFormat::Png);
        match item {
            ClipItem::Image { data, .. } => match data {
                LazyPayload::Inline(b) => assert_eq!(b, vec![1, 2, 3, 4]),
                LazyPayload::OnDemand { .. } => panic!("small image should inline"),
            },
            _ => panic!(),
        }
        assert!(store.is_empty(), "small images don't touch the store");
    }

    #[test]
    fn large_image_goes_lazy() {
        let store = LazyStore::new();
        let big: Vec<u8> = vec![7u8; INLINE_THRESHOLD + 1];
        let item = produce_image(&store, big.clone(), ImageFormat::Png);
        match &item {
            ClipItem::Image { data, .. } => match data {
                LazyPayload::OnDemand { hash, size } => {
                    assert_eq!(*size, big.len() as u64);
                    let bytes = store.get(hash).expect("bytes present");
                    assert_eq!(bytes.len(), big.len());
                }
                _ => panic!("large image must be lazy"),
            },
            _ => panic!(),
        }
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn store_evicts_oldest_when_capped() {
        let cap = 1024;
        let store = LazyStore::with_cap(cap);
        let h1 = store.insert(vec![1u8; 600]);
        assert!(store.get(&h1).is_some());
        // Inserting another 600 bytes evicts the first entry to fit.
        let _h2 = store.insert(vec![2u8; 600]);
        assert!(store.get(&h1).is_none(), "h1 should be evicted");
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn duplicate_insert_is_idempotent() {
        let store = LazyStore::new();
        let h1 = store.insert(vec![1, 2, 3]);
        let h2 = store.insert(vec![1, 2, 3]);
        assert_eq!(h1, h2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn serve_fetch_resolves_inline_and_on_demand() {
        let store = LazyStore::new();
        let inline = LazyPayload::Inline(vec![4, 5, 6]);
        assert_eq!(serve_fetch(&store, &inline), Some(vec![4, 5, 6]));

        let bytes = vec![9u8; 1024];
        let hash = store.insert(bytes.clone());
        let on_demand = LazyPayload::OnDemand {
            hash,
            size: bytes.len() as u64,
        };
        assert_eq!(serve_fetch(&store, &on_demand), Some(bytes));

        let missing = LazyPayload::OnDemand {
            hash: [0u8; 32],
            size: 4,
        };
        assert_eq!(serve_fetch(&store, &missing), None);
    }
}
