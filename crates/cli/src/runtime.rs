//! `borderless start` runtime: wires the transport, clipboard PAL, and
//! sync engine together.

use anyhow::{Context, Result};
use borderless_clipboard::{Decision, Engine as ClipboardEngine};
use borderless_core::WireFrame;
use borderless_pal::Clipboard as ClipboardTrait;
use borderless_transport::discovery::{announce_and_browse, DiscoveredPeer};
use borderless_transport::{Connection, Endpoint, EndpointConfig, Identity, PeerStore};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::config::Config;

/// Glue everything for the daemon mode.
pub struct Runtime {
    pub identity: Identity,
    #[allow(dead_code)] // referenced via Endpoint internally
    pub peers: Arc<Mutex<PeerStore>>,
    pub endpoint: Arc<Endpoint>,
    pub clipboard: ClipboardEngine,
}

impl Runtime {
    /// Construct from on-disk state.
    pub async fn bootstrap(cfg: &Config, config_dir: &Path, allow_new_peers: bool) -> Result<Self> {
        let identity =
            Identity::load_or_generate(config_dir.join("identity.key")).context("load identity")?;
        let peers = Arc::new(Mutex::new(
            PeerStore::open(config_dir.join("known_peers.toml")).context("open peer store")?,
        ));

        let endpoint_cfg = EndpointConfig {
            bind: cfg.network.bind_addr(),
            name: cfg.node.name.clone(),
            allow_new_peers,
        };
        let endpoint = Endpoint::bind(identity.clone(), peers.clone(), endpoint_cfg)
            .context("bind quic endpoint")?;
        let endpoint = Arc::new(endpoint);

        let clipboard = ClipboardEngine::new(identity.node_id(), cfg.clipboard.history_size.max(1));

        Ok(Self {
            identity,
            peers,
            endpoint,
            clipboard,
        })
    }

    /// Run until interrupted.
    pub async fn run(self, cfg: &Config) -> Result<()> {
        let local_addr = self.endpoint.local_addr()?;
        info!(node = %self.identity.node_id(), %local_addr, name = %cfg.node.name, "started");

        // Spawn discovery; we keep the handle alive via the Drop on
        // shutdown.
        let (_disc_handle, mut peer_rx) = announce_and_browse(
            &cfg.node.name,
            self.identity.node_id(),
            local_addr.port(),
            &cfg.node.name,
        )
        .context("start mDNS")?;

        let endpoint = self.endpoint.clone();
        let clipboard = self.clipboard.clone();

        // Accept loop.
        let acc_endpoint = endpoint.clone();
        let acc_clipboard = clipboard.clone();
        tokio::spawn(async move {
            loop {
                match acc_endpoint.accept().await {
                    Ok(conn) => {
                        info!(peer = %conn.peer_node_id, name = %conn.peer_name, "accepted");
                        spawn_connection_loop(conn, acc_clipboard.clone());
                    }
                    Err(e) => {
                        warn!(error = %e, "accept failed; sleeping briefly");
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        });

        // Clipboard watch loop: every local clipboard change becomes a
        // snapshot broadcast to every connected peer (v0.1 keeps a
        // single most-recent connection per peer; we re-dial as
        // peers are discovered).
        let pal_clipboard = open_local_clipboard()?;
        let connections = Arc::new(parking_lot::Mutex::new(Vec::<Arc<Connection>>::new()));
        if let Some(mut pal_clip) = pal_clipboard {
            let watcher = pal_clip.watch();
            let connections = connections.clone();
            let clipboard = clipboard.clone();
            tokio::spawn(local_clipboard_loop(watcher, clipboard, connections));
        } else {
            warn!("no local clipboard backend; clipboard sync will not run");
        }

        // Peer-discovery loop: connect to every newly-resolved peer.
        let endpoint_for_dial = endpoint.clone();
        let connections_for_dial = connections.clone();
        let dial_clipboard = clipboard.clone();
        let self_node_id = self.identity.node_id();
        tokio::spawn(async move {
            while let Some(peer) = peer_rx.recv().await {
                if peer.node_id == Some(self_node_id) {
                    continue;
                }
                match dial_peer(&endpoint_for_dial, &peer).await {
                    Ok(conn) => {
                        info!(peer = %conn.peer_node_id, name = %conn.peer_name, "connected");
                        let conn = Arc::new(conn);
                        connections_for_dial.lock().push(conn.clone());
                        spawn_connection_loop_arc(conn, dial_clipboard.clone());
                    }
                    Err(e) => warn!(error = %e, name = %peer.name, "dial failed"),
                }
            }
        });

        tokio::signal::ctrl_c().await.ok();
        info!("shutting down");
        endpoint.close().await;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn open_local_clipboard() -> Result<Option<Box<dyn ClipboardTrait>>> {
    match borderless_pal_x11::X11Clipboard::new() {
        Ok(c) => Ok(Some(Box::new(c))),
        Err(e) => {
            warn!(error = %e, "X11 clipboard unavailable");
            Ok(None)
        }
    }
}

#[cfg(target_os = "windows")]
fn open_local_clipboard() -> Result<Option<Box<dyn ClipboardTrait>>> {
    match borderless_pal_windows::WindowsClipboard::new() {
        Ok(c) => Ok(Some(Box::new(c))),
        Err(e) => {
            warn!(error = %e, "Windows clipboard unavailable");
            Ok(None)
        }
    }
}

#[cfg(target_os = "macos")]
fn open_local_clipboard() -> Result<Option<Box<dyn ClipboardTrait>>> {
    match borderless_pal_macos::MacosClipboard::new() {
        Ok(c) => Ok(Some(Box::new(c))),
        Err(e) => {
            warn!(error = %e, "macOS clipboard unavailable");
            Ok(None)
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn open_local_clipboard() -> Result<Option<Box<dyn ClipboardTrait>>> {
    Ok(None)
}

async fn local_clipboard_loop(
    mut watcher: mpsc::UnboundedReceiver<String>,
    clipboard: ClipboardEngine,
    connections: Arc<parking_lot::Mutex<Vec<Arc<Connection>>>>,
) {
    while let Some(text) = watcher.recv().await {
        let snap = clipboard.produce_text(text);
        let frame = WireFrame::Clipboard(snap);
        let conns: Vec<Arc<Connection>> = connections.lock().clone();
        for c in conns {
            if let Err(e) = c.send_frame(&frame).await {
                warn!(error = %e, peer = %c.peer_node_id, "send clipboard failed");
            }
        }
    }
}

fn spawn_connection_loop(conn: Connection, clipboard: ClipboardEngine) {
    let conn = Arc::new(conn);
    spawn_connection_loop_arc(conn, clipboard);
}

fn spawn_connection_loop_arc(conn: Arc<Connection>, clipboard: ClipboardEngine) {
    tokio::spawn(async move {
        loop {
            match conn.recv_frame().await {
                Ok(WireFrame::Clipboard(snap)) => match clipboard.observe_remote(snap) {
                    Decision::Apply(snap) => {
                        if let Err(e) = apply_clipboard_snapshot(&snap).await {
                            warn!(error = %e, "apply clipboard failed");
                        }
                    }
                    Decision::Ignore => {}
                },
                Ok(WireFrame::Control(_)) => {
                    // v0.1: ignored; ping/pong/control will be wired in v0.2.
                }
                Ok(WireFrame::Input(_ev)) => {
                    // v0.1: input emit is unimplemented in PAL stubs.
                }
                Err(e) => {
                    warn!(error = %e, peer = %conn.peer_node_id, "connection ended");
                    break;
                }
            }
        }
    });
}

async fn apply_clipboard_snapshot(snap: &borderless_core::ClipboardSnapshot) -> Result<()> {
    if let Some(mut clip) = open_local_clipboard()? {
        clip.write(snap)
            .await
            .map_err(|e| anyhow::anyhow!("write clipboard: {e}"))?;
    }
    Ok(())
}

async fn dial_peer(endpoint: &Endpoint, peer: &DiscoveredPeer) -> Result<Connection> {
    let mut last_err = None;
    for ip in &peer.addrs {
        let addr = std::net::SocketAddr::new(*ip, peer.port);
        match endpoint.connect(addr).await {
            Ok(conn) => return Ok(conn),
            Err(e) => {
                last_err = Some(e);
            }
        }
    }
    if let Some(e) = last_err {
        Err(e.into())
    } else {
        anyhow::bail!("peer has no addresses")
    }
}

#[allow(unused_imports)]
use error as _err_unused;
