//! `borderless` CLI entry point.

mod config;
mod doctor;
mod runtime;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use tracing::info;
use tracing_subscriber::EnvFilter;

use config::{Config, RoleKind};

#[derive(Parser, Debug)]
#[command(
    name = "borderless",
    version,
    about = "Cross-platform LAN keyboard / mouse / clipboard sharing (C/S)"
)]
struct Cli {
    /// Override the config directory (default: per-platform XDG-ish).
    #[arg(long, env = "BORDERLESS_CONFIG_DIR")]
    config_dir: Option<PathBuf>,

    /// Verbose logging (`-v` info, `-vv` debug, `-vvv` trace).
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run as the Hub (server). Binds a TCP+TLS listener and accepts spokes.
    Serve {
        /// Override `config.toml`'s `[hub].bind_ip` / `[hub].port`.
        #[arg(long)]
        bind: Option<String>,
        /// Accept brand-new spokes on this run (TOFU).
        #[arg(long)]
        accept_new_peers: bool,
    },
    /// Run as a Spoke (client). Dials a Hub.
    ///
    /// With `<host:port>`, persists the address to `config.toml`'s
    /// `[client].server_addr`. Without arguments, reads it from there.
    Connect {
        /// Hub address `host:port`.
        host_port: Option<String>,
        /// First-time pair: TOFU-trust the hub's pubkey if not yet known.
        #[arg(long)]
        pair: bool,
        /// Optional pinned hub pubkey (32 bytes hex).
        #[arg(long)]
        pin: Option<String>,
    },
    /// Print local node id, role, and known peers.
    Status,
    /// Inspect or manipulate the clipboard.
    #[command(subcommand)]
    Clip(ClipCmd),
    /// Diagnose platform permissions / environment.
    Doctor,
}

#[derive(Subcommand, Debug)]
enum ClipCmd {
    /// Show the most recent N snapshots seen by this node.
    History {
        /// Limit.
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
    /// Write `text` to the local OS clipboard (mostly useful for tests
    /// and demos on headless / minimal-tooling machines).
    Set {
        /// Text to put on the clipboard.
        text: String,
    },
    /// Read the local OS clipboard and print it to stdout.
    Get,
}

fn init_tracing(verbose: u8) {
    let default_level = match verbose {
        0 => "borderless=info,warn",
        1 => "borderless=debug,info",
        2 => "borderless=trace,debug",
        _ => "trace",
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}

fn resolve_config_dir(cli: &Cli) -> Result<PathBuf> {
    if let Some(d) = &cli.config_dir {
        return Ok(d.clone());
    }
    config::default_config_dir()
}

#[cfg(target_os = "linux")]
fn open_local_clipboard_now() -> Result<Box<dyn borderless_pal::Clipboard>> {
    let c = borderless_pal_x11::X11Clipboard::new()
        .map_err(|e| anyhow::anyhow!("open clipboard: {e}"))?;
    Ok(Box::new(c))
}

#[cfg(target_os = "windows")]
fn open_local_clipboard_now() -> Result<Box<dyn borderless_pal::Clipboard>> {
    let c = borderless_pal_windows::WindowsClipboard::new()
        .map_err(|e| anyhow::anyhow!("open clipboard: {e}"))?;
    Ok(Box::new(c))
}

#[cfg(target_os = "macos")]
fn open_local_clipboard_now() -> Result<Box<dyn borderless_pal::Clipboard>> {
    let c = borderless_pal_macos::MacosClipboard::new()
        .map_err(|e| anyhow::anyhow!("open clipboard: {e}"))?;
    Ok(Box::new(c))
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn open_local_clipboard_now() -> Result<Box<dyn borderless_pal::Clipboard>> {
    anyhow::bail!("clipboard not supported on this platform")
}

/// Write `text` to the host clipboard with platform-correct
/// persistence. On X11 the contents live only as long as the owning
/// process — `arboard::SetExtLinux::wait()` blocks until any other
/// client claims the selection, then we exit. On macOS and Windows
/// the OS itself owns clipboard storage, so a one-shot write is
/// sufficient.
#[cfg(target_os = "linux")]
async fn clip_set(text: String) -> Result<()> {
    use arboard::{Clipboard, SetExtLinux};
    let len = text.len();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let mut clip = Clipboard::new().context("open X11 clipboard")?;
        clip.set()
            .wait()
            .text(text)
            .context("write & hold clipboard ownership")?;
        Ok(())
    })
    .await
    .context("clipboard task panicked")??;
    println!("wrote {len} bytes; ownership released after another client read the selection");
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
async fn clip_set(text: String) -> Result<()> {
    let len = text.len();
    let mut c = open_local_clipboard_now()?;
    let snap = borderless_core::ClipboardSnapshot {
        version: 1,
        origin: borderless_core::NodeId([0; 16]),
        created_unix_ms: 0,
        items: vec![borderless_core::ClipItem::Text(text)],
    };
    c.write(&snap)
        .await
        .map_err(|e| anyhow::anyhow!("write clipboard: {e}"))?;
    println!("wrote {len} bytes to clipboard");
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
async fn clip_set(_text: String) -> Result<()> {
    anyhow::bail!("clip set not supported on this platform")
}

fn maybe_override_bind(cfg: &mut Config, bind: Option<String>) -> Result<()> {
    if let Some(b) = bind {
        let sa: std::net::SocketAddr = b.parse().with_context(|| format!("parse --bind {b}"))?;
        cfg.hub.bind_ip = sa.ip();
        cfg.hub.port = sa.port();
    }
    Ok(())
}

fn ensure_role(cfg: &mut Config, dir: &Path, want: RoleKind) -> Result<()> {
    if cfg.role.kind != want {
        cfg.role.kind = want;
        config::save(dir, cfg)?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let config_dir = resolve_config_dir(&cli)?;
    let mut cfg = config::load_or_default(&config_dir)?;

    match cli.command {
        Command::Serve {
            bind,
            accept_new_peers,
        } => {
            maybe_override_bind(&mut cfg, bind)?;
            ensure_role(&mut cfg, &config_dir, RoleKind::Hub)?;
            let rt = runtime::ServerRuntime::bootstrap(
                &cfg,
                &config_dir,
                if accept_new_peers { Some(true) } else { None },
            )
            .await
            .context("bootstrap hub")?;
            info!(
                bind = %cfg.hub.bind_addr(),
                accept_new_peers,
                "starting hub"
            );
            rt.run(&cfg).await
        }
        Command::Connect {
            host_port,
            pair,
            pin,
        } => {
            // Resolve effective server address: argument → config.
            let server_addr = match host_port {
                Some(h) => {
                    cfg.client.server_addr = Some(h.clone());
                    ensure_role(&mut cfg, &config_dir, RoleKind::Spoke)?;
                    h
                }
                None => match cfg.client.server_addr.clone() {
                    Some(s) => s,
                    None => {
                        anyhow::bail!(
                            "no server address: pass `borderless connect <host:port>` once to set it"
                        );
                    }
                },
            };
            let expected = pin.or_else(|| cfg.client.expected_server_id.clone());
            if let Some(p) = &expected {
                cfg.client.expected_server_id = Some(p.clone());
                config::save(&config_dir, &cfg)?;
            }

            ensure_role(&mut cfg, &config_dir, RoleKind::Spoke)?;
            let rt =
                runtime::ClientRuntime::bootstrap(&cfg, &config_dir, server_addr, expected, pair)
                    .context("bootstrap spoke")?;
            rt.run(&cfg).await
        }
        Command::Status => {
            let identity =
                borderless_transport::Identity::load_or_generate(config_dir.join("identity.key"))?;
            let store = borderless_transport::PeerStore::open(config_dir.join("known_peers.toml"))?;
            println!("node_id   : {}", identity.node_id());
            println!("name      : {}", cfg.node.name);
            println!("role      : {:?}", cfg.role.kind);
            match cfg.role.kind {
                RoleKind::Hub => {
                    println!("bind      : {}", cfg.hub.bind_addr());
                    println!("accept_new: {}", cfg.hub.accept_new_peers);
                }
                RoleKind::Spoke => {
                    println!(
                        "server    : {}",
                        cfg.client.server_addr.as_deref().unwrap_or("(unset)")
                    );
                    if let Some(p) = &cfg.client.expected_server_id {
                        println!("pinned    : {}", &p[..p.len().min(16)]);
                    }
                }
                RoleKind::Unconfigured => {
                    println!("(run `borderless serve` or `borderless connect <addr>` first)");
                }
            }
            println!("config_dir: {}", config_dir.display());
            println!("peers     : {}", store.len());
            for p in store.iter() {
                println!("  - {}  ({})", p.name, &p.pubkey[..16]);
            }
            Ok(())
        }
        Command::Clip(ClipCmd::History { limit }) => {
            let _ = limit;
            println!(
                "no live IPC yet — run `borderless serve` or `borderless connect` and copy text to populate the daemon's history.\n\
                 v0.3 will expose this over a local UDS bridge."
            );
            Ok(())
        }
        Command::Clip(ClipCmd::Set { text }) => clip_set(text).await,
        Command::Clip(ClipCmd::Get) => {
            let mut c = open_local_clipboard_now()?;
            match c
                .read()
                .await
                .map_err(|e| anyhow::anyhow!("read clipboard: {e}"))?
            {
                Some(text) => {
                    println!("{text}");
                }
                None => {
                    eprintln!("(clipboard is empty or non-text)");
                    std::process::exit(1);
                }
            }
            Ok(())
        }
        Command::Doctor => {
            let report = doctor::run(&cfg, &config_dir)?;
            println!("{}", doctor::format(&report));
            Ok(())
        }
    }
}
