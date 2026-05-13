//! Hub-side runtime.
//!
//! Responsibilities:
//!
//! - Bind a TCP+TLS listener and authenticate spokes through the
//!   `SignedHello` handshake.
//! - Maintain a map `NodeId -> writer-task channel` for all live spokes.
//! - Forward local clipboard changes to every connected spoke.
//! - Re-broadcast spoke clipboard updates to every other spoke
//!   (and apply them locally).
//! - Reject spoke-originated `WireFrame::Input` (Hub-only-active).
//! - Input capture from local PAL is wired in but the v0.2 PAL stubs
//!   never produce events; once the real captures land they slot in
//!   without runtime changes.

use anyhow::{Context, Result};
use borderless_clipboard::{Engine as ClipboardEngine, LazyStore};
use borderless_core::{InputEvent, NodeId, WireFrame};
use borderless_pal::{CaptureMode, EventSink, InputCapture as _};
use borderless_transport::{Connection, Identity, Listener, ListenerConfig, PeerStore};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::runtime::common::{
    handle_remote_clipboard, open_local_clipboard, spawn_local_clipboard_watcher,
};

/// Per-spoke writer channel.
type WriterMap = Arc<Mutex<HashMap<NodeId, mpsc::UnboundedSender<WireFrame>>>>;

/// Hub runtime.
pub struct ServerRuntime {
    /// Long-term identity.
    pub identity: Identity,
    /// Persistent peer store.
    #[allow(dead_code)]
    pub peers: Arc<Mutex<PeerStore>>,
    /// TLS listener.
    pub listener: Arc<Listener>,
    /// Clipboard sync engine.
    pub clipboard: ClipboardEngine,
    /// Lazy payload store for images and other oversized clipboard items.
    pub lazy: LazyStore,
}

impl ServerRuntime {
    /// Construct from on-disk state. `accept_new_peers_override`, if
    /// `Some`, replaces the value from `cfg.hub.accept_new_peers`
    /// for this run only (used by `borderless serve --accept-new-peers`).
    pub async fn bootstrap(
        cfg: &Config,
        config_dir: &Path,
        accept_new_peers_override: Option<bool>,
    ) -> Result<Self> {
        let identity =
            Identity::load_or_generate(config_dir.join("identity.key")).context("load identity")?;
        let peers = Arc::new(Mutex::new(
            PeerStore::open(config_dir.join("known_peers.toml")).context("open peer store")?,
        ));

        let listener_cfg = ListenerConfig {
            bind: cfg.hub.bind_addr(),
            name: cfg.node.name.clone(),
            accept_new_peers: accept_new_peers_override.unwrap_or(cfg.hub.accept_new_peers),
        };
        let listener = Listener::bind(identity.clone(), peers.clone(), listener_cfg)
            .await
            .context("bind tcp+tls listener")?;
        let listener = Arc::new(listener);

        let clipboard = ClipboardEngine::new(identity.node_id(), cfg.clipboard.history_size.max(1));
        let lazy = LazyStore::new();

        Ok(Self {
            identity,
            peers,
            listener,
            clipboard,
            lazy,
        })
    }

    /// Run until interrupted (Ctrl-C).
    pub async fn run(self, _cfg: &Config) -> Result<()> {
        let local_addr = self.listener.local_addr()?;
        info!(
            node = %self.identity.node_id(),
            %local_addr,
            "hub listening"
        );

        let writers: WriterMap = Arc::new(Mutex::new(HashMap::new()));

        // Local clipboard watcher: every local change becomes a frame
        // broadcast to all spokes.
        let mut local_watcher_rx = if let Some(mut clip) = open_local_clipboard()? {
            let raw_rx = clip.watch();
            spawn_local_clipboard_watcher(raw_rx, self.clipboard.clone())
        } else {
            warn!("no local clipboard backend; clipboard sync inbound from spokes only");
            // Empty receiver; the broadcast loop will simply see EOF
            // and exit. Use a placeholder channel so the type lines up.
            let (tx, rx) = mpsc::unbounded_channel();
            drop(tx);
            rx
        };

        let writers_for_clip = writers.clone();
        tokio::spawn(async move {
            while let Some(frame) = local_watcher_rx.recv().await {
                broadcast_to_all(&writers_for_clip, &frame, None);
            }
        });

        // Local input capture: Hub-only-active. v0.2 PAL stubs don't
        // actually emit events; this is here so the real capture
        // backend slots in cleanly when it lands.
        let writers_for_input = writers.clone();
        let (capture_tx, mut capture_rx) = mpsc::unbounded_channel::<InputEvent>();
        tokio::spawn(async move {
            while let Some(ev) = capture_rx.recv().await {
                let frame = WireFrame::Input(ev);
                broadcast_to_all(&writers_for_input, &frame, None);
            }
        });
        if let Err(e) = start_local_input_capture(capture_tx).await {
            // Capture is optional; without it the hub still works as a
            // pure clipboard relay.
            debug!(error = %e, "local input capture not started");
        }

        // Accept loop.
        let listener = self.listener.clone();
        let clipboard = self.clipboard.clone();
        let lazy = self.lazy.clone();
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("received ctrl-c, shutting down hub");
                    break;
                }
                accept = listener.accept() => match accept {
                    Ok(conn) => {
                        info!(
                            peer = %conn.peer_node_id,
                            name = %conn.peer_name,
                            "spoke connected"
                        );
                        spawn_spoke_session(
                            Arc::new(conn),
                            writers.clone(),
                            clipboard.clone(),
                            lazy.clone(),
                        );
                    }
                    Err(e) => {
                        warn!(error = %e, "accept failed");
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Best-effort kick-off of the platform input capture. Today this is
/// always a no-op (the PAL backends are stubs); once real implementations
/// land they'll start producing events into `sink`.
async fn start_local_input_capture(_sink: EventSink) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let mut cap = borderless_pal_x11::X11Capture::new();
        cap.set_mode(CaptureMode::Listen)
            .await
            .map_err(|e| anyhow::anyhow!("set capture mode: {e}"))?;
        cap.start(_sink)
            .await
            .map_err(|e| anyhow::anyhow!("start capture: {e}"))?;
        // Keep the capture alive via leaking; a real impl will hook
        // shutdown later. v0.2 stubs are inert so this leak is fine.
        Box::leak(Box::new(cap));
    }
    #[cfg(target_os = "windows")]
    {
        let mut cap = borderless_pal_windows::WindowsCapture::new();
        cap.set_mode(CaptureMode::Listen)
            .await
            .map_err(|e| anyhow::anyhow!("set capture mode: {e}"))?;
        cap.start(_sink)
            .await
            .map_err(|e| anyhow::anyhow!("start capture: {e}"))?;
        Box::leak(Box::new(cap));
    }
    #[cfg(target_os = "macos")]
    {
        let mut cap = borderless_pal_macos::MacosCapture::new();
        cap.set_mode(CaptureMode::Listen)
            .await
            .map_err(|e| anyhow::anyhow!("set capture mode: {e}"))?;
        cap.start(_sink)
            .await
            .map_err(|e| anyhow::anyhow!("start capture: {e}"))?;
        Box::leak(Box::new(cap));
    }
    Ok(())
}

/// Maximum bytes per `FetchResponse` chunk on the wire. Postcard
/// length-prefix overhead is small; 256 KiB keeps memory bounded and
/// each frame fits well under the 64 MiB protocol cap.
const FETCH_CHUNK_SIZE: usize = 256 * 1024;

fn chunk_fetch_response(hash: [u8; 32], bytes: &[u8]) -> Vec<WireFrame> {
    if bytes.is_empty() {
        return vec![WireFrame::FetchResponse {
            hash,
            chunk_idx: 0,
            total: 1,
            bytes: Vec::new(),
        }];
    }
    let total = bytes.len().div_ceil(FETCH_CHUNK_SIZE) as u32;
    bytes
        .chunks(FETCH_CHUNK_SIZE)
        .enumerate()
        .map(|(i, chunk)| WireFrame::FetchResponse {
            hash,
            chunk_idx: i as u32,
            total,
            bytes: chunk.to_vec(),
        })
        .collect()
}

fn broadcast_to_all(writers: &WriterMap, frame: &WireFrame, exclude: Option<NodeId>) {
    let map = writers.lock();
    for (peer_id, tx) in map.iter() {
        if Some(*peer_id) == exclude {
            continue;
        }
        if let Err(e) = tx.send(frame.clone()) {
            debug!(peer = %peer_id, error = %e, "drop frame: writer closed");
        }
    }
}

fn spawn_spoke_session(
    conn: Arc<Connection>,
    writers: WriterMap,
    clipboard: ClipboardEngine,
    lazy: LazyStore,
) {
    let peer_id = conn.peer_node_id;

    // Per-spoke writer channel.
    let (tx, mut rx) = mpsc::unbounded_channel::<WireFrame>();
    {
        let mut map = writers.lock();
        if let Some(existing) = map.insert(peer_id, tx) {
            // Preexisting connection's writer drops, eventually closing
            // the old session. The new session takes over cleanly.
            drop(existing);
            warn!(peer = %peer_id, "replaced existing spoke session");
        }
    }

    // Writer task: pulls frames off the channel and ships them.
    let conn_w = conn.clone();
    let peer_id_w = peer_id;
    tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            if let Err(e) = conn_w.send_frame(&frame).await {
                debug!(peer = %peer_id_w, error = %e, "writer send failed; closing");
                conn_w.close().await;
                break;
            }
        }
    });

    // Reader task: applies clipboard, fans out to other spokes,
    // rejects spoke-sent input.
    let writers_for_read = writers.clone();
    tokio::spawn(async move {
        loop {
            match conn.recv_frame().await {
                Ok(frame) => match frame {
                    WireFrame::Clipboard(snap) => {
                        let echoed = handle_remote_clipboard(&clipboard, snap.clone()).await;
                        if echoed.is_some() {
                            // Fan-out to other spokes.
                            broadcast_to_all(
                                &writers_for_read,
                                &WireFrame::Clipboard(snap),
                                Some(peer_id),
                            );
                        }
                    }
                    WireFrame::Input(_) => {
                        warn!(peer = %peer_id, "spoke sent input frame; dropping (Hub-only-active)");
                    }
                    WireFrame::FetchRequest { hash } => {
                        let reply = match lazy.get(&hash) {
                            Some(bytes) => chunk_fetch_response(hash, &bytes),
                            None => vec![WireFrame::FetchMiss { hash }],
                        };
                        if let Some(tx) = writers_for_read.lock().get(&peer_id) {
                            for f in reply {
                                let _ = tx.send(f);
                            }
                        }
                    }
                    WireFrame::FetchResponse { .. } | WireFrame::FetchMiss { .. } => {
                        // Hub doesn't currently issue fetches.
                        debug!(peer = %peer_id, "ignoring unexpected fetch frame");
                    }
                    WireFrame::Control(_) => {
                        // Ping/Pong/Hello/Bye plumbed in a later step.
                    }
                },
                Err(e) => {
                    info!(peer = %peer_id, error = %e, "spoke session ended");
                    break;
                }
            }
        }
        // Remove from writers map if our entry is still the one we
        // installed.
        writers_for_read.lock().remove(&peer_id);
    });
}
