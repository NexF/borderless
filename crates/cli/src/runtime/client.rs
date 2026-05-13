//! Spoke-side runtime.
//!
//! Behaviour:
//!
//! - Dial the configured Hub. On failure, retry with exponential
//!   backoff (1s → 2s → 5s → 15s → 60s, then steady at 60s).
//! - On a live session, run two tasks: a reader that applies clipboard
//!   updates locally and (when input is enabled) injects input events,
//!   and a writer that ships local clipboard changes to the Hub.
//! - The Spoke does NOT capture local input. Hub-only-active by design.

use anyhow::{Context, Result};
use borderless_clipboard::Engine as ClipboardEngine;
use borderless_core::WireFrame;
use borderless_transport::{Connection, Connector, Identity, PeerStore};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::lookup_host;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::runtime::common::{
    handle_remote_clipboard, open_local_clipboard, spawn_local_clipboard_watcher,
};

/// Spoke runtime.
pub struct ClientRuntime {
    /// Long-term identity.
    pub identity: Identity,
    /// Persistent peer store.
    #[allow(dead_code)]
    pub peers: Arc<Mutex<PeerStore>>,
    /// TLS connector.
    pub connector: Connector,
    /// Clipboard sync engine.
    pub clipboard: ClipboardEngine,
    /// Hub address (host:port).
    pub server_addr: String,
    /// Optional pinned hub pubkey.
    pub expected_server_pubkey: Option<[u8; 32]>,
    /// True iff the user passed `--pair` for this run (TOFU first time).
    pub accept_new_peer: bool,
    /// Spoke-local input enabled?
    pub input_enabled: bool,
}

impl ClientRuntime {
    /// Construct from on-disk state plus per-run overrides.
    pub fn bootstrap(
        cfg: &Config,
        config_dir: &Path,
        server_addr: String,
        expected_server_id: Option<String>,
        accept_new_peer: bool,
    ) -> Result<Self> {
        let identity =
            Identity::load_or_generate(config_dir.join("identity.key")).context("load identity")?;
        let peers = Arc::new(Mutex::new(
            PeerStore::open(config_dir.join("known_peers.toml")).context("open peer store")?,
        ));

        let connector = Connector::new(identity.clone(), peers.clone(), cfg.node.name.clone())
            .context("build connector")?;

        let clipboard = ClipboardEngine::new(identity.node_id(), cfg.clipboard.history_size.max(1));

        let expected_server_pubkey = match expected_server_id {
            Some(s) => Some(parse_pubkey(&s).context("parse expected_server_id")?),
            None => None,
        };

        Ok(Self {
            identity,
            peers,
            connector,
            clipboard,
            server_addr,
            expected_server_pubkey,
            accept_new_peer,
            input_enabled: cfg.input.enabled,
        })
    }

    /// Run until interrupted.
    pub async fn run(self, _cfg: &Config) -> Result<()> {
        info!(
            node = %self.identity.node_id(),
            server = %self.server_addr,
            "spoke starting"
        );

        let backoff = [1, 2, 5, 15, 60];
        let mut idx = 0usize;

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("ctrl-c, shutting down spoke");
                    return Ok(());
                }
                res = run_once(&self) => {
                    match res {
                        Ok(()) => {
                            info!("session ended cleanly; reconnecting");
                            idx = 0;
                        }
                        Err(e) => {
                            let delay = backoff[idx.min(backoff.len() - 1)];
                            warn!(error = %e, retry_in_secs = delay, "session failed; will retry");
                            tokio::select! {
                                _ = tokio::signal::ctrl_c() => return Ok(()),
                                _ = tokio::time::sleep(Duration::from_secs(delay)) => {}
                            }
                            idx = (idx + 1).min(backoff.len() - 1);
                        }
                    }
                }
            }
        }
    }
}

fn parse_pubkey(s: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(s).context("hex decode")?;
    if bytes.len() != 32 {
        anyhow::bail!("expected 32-byte pubkey, got {}", bytes.len());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// One full attempt: resolve, dial, run reader+writer, return.
async fn run_once(rt: &ClientRuntime) -> Result<()> {
    // Resolve "host:port" via DNS / /etc/hosts. tokio::net::lookup_host
    // returns an iterator of SocketAddrs; we try each in order.
    let mut addrs: Vec<std::net::SocketAddr> = lookup_host(&rt.server_addr)
        .await
        .with_context(|| format!("resolve {}", rt.server_addr))?
        .collect();
    if addrs.is_empty() {
        anyhow::bail!("no addresses for {}", rt.server_addr);
    }
    // Prefer IPv4 first; many home networks have broken IPv6 paths.
    addrs.sort_by_key(|a| if a.is_ipv4() { 0 } else { 1 });

    let mut last_err: Option<anyhow::Error> = None;
    for addr in addrs {
        debug!(%addr, "dialing");
        match rt
            .connector
            .dial(addr, rt.expected_server_pubkey, rt.accept_new_peer)
            .await
        {
            Ok(conn) => {
                info!(
                    server = %conn.peer_node_id,
                    name = %conn.peer_name,
                    "connected to hub"
                );
                let conn = Arc::new(conn);
                run_session(rt, conn).await?;
                return Ok(());
            }
            Err(e) => {
                debug!(%addr, error = %e, "dial candidate failed");
                last_err = Some(anyhow::anyhow!(e));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no candidate addresses worked")))
}

async fn run_session(rt: &ClientRuntime, conn: Arc<Connection>) -> Result<()> {
    let (write_tx, mut write_rx) = mpsc::unbounded_channel::<WireFrame>();

    // Local clipboard watcher: each change becomes a WireFrame::Clipboard
    // queued onto write_tx.
    if let Some(mut clip) = open_local_clipboard()? {
        let raw_rx = clip.watch();
        let mut frame_rx = spawn_local_clipboard_watcher(raw_rx, rt.clipboard.clone());
        let write_tx_clip = write_tx.clone();
        tokio::spawn(async move {
            while let Some(frame) = frame_rx.recv().await {
                if write_tx_clip.send(frame).is_err() {
                    break;
                }
            }
        });
    } else {
        warn!("no local clipboard backend; outbound clipboard sync disabled");
    }

    // Writer task.
    let conn_w = conn.clone();
    let writer = tokio::spawn(async move {
        while let Some(frame) = write_rx.recv().await {
            if let Err(e) = conn_w.send_frame(&frame).await {
                debug!(error = %e, "spoke writer send failed");
                conn_w.close().await;
                return;
            }
        }
    });

    // Reader loop on the current task.
    let clipboard = rt.clipboard.clone();
    let input_enabled = rt.input_enabled;

    let read_result = reader_loop(conn.clone(), clipboard, input_enabled).await;

    // Drain the writer.
    drop(write_tx);
    let _ = writer.await;

    read_result
}

async fn reader_loop(
    conn: Arc<Connection>,
    clipboard: ClipboardEngine,
    input_enabled: bool,
) -> Result<()> {
    let mut emit = if input_enabled {
        open_input_emit().await
    } else {
        None
    };

    loop {
        let frame = conn.recv_frame().await?;
        match frame {
            WireFrame::Clipboard(snap) => {
                handle_remote_clipboard(&clipboard, snap).await;
            }
            WireFrame::Input(ev) => {
                if let Some(em) = emit.as_mut() {
                    if let Err(e) = em.emit(ev).await {
                        debug!(error = %e, "input emit failed");
                    }
                }
            }
            WireFrame::FetchRequest { hash } => {
                // Spokes don't host a LazyStore yet; reply miss.
                let _ = conn.send_frame(&WireFrame::FetchMiss { hash }).await;
            }
            WireFrame::FetchResponse { .. } | WireFrame::FetchMiss { .. } => {
                // Reserved for image_clipboard step.
            }
            WireFrame::Control(_) => {
                // Ping/Pong/Hello/Bye in a later step.
            }
        }
    }
}

#[cfg(target_os = "linux")]
async fn open_input_emit() -> Option<Box<dyn borderless_pal::InputEmit>> {
    match borderless_pal_x11::X11Emit::new() {
        Ok(e) => Some(Box::new(e)),
        Err(err) => {
            warn!(error = %err, "X11Emit::new failed; spoke input disabled");
            None
        }
    }
}

#[cfg(target_os = "windows")]
async fn open_input_emit() -> Option<Box<dyn borderless_pal::InputEmit>> {
    Some(Box::new(borderless_pal_windows::WindowsEmit::new()))
}

#[cfg(target_os = "macos")]
async fn open_input_emit() -> Option<Box<dyn borderless_pal::InputEmit>> {
    match borderless_pal_macos::MacosEmit::new() {
        Ok(e) => Some(Box::new(e)),
        Err(err) => {
            warn!(error = %err, "MacosEmit::new failed; spoke input disabled");
            None
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
async fn open_input_emit() -> Option<Box<dyn borderless_pal::InputEmit>> {
    None
}
