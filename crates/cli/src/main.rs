//! `borderless` CLI entry point.

mod config;
mod doctor;
mod runtime;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "borderless",
    version,
    about = "Cross-platform LAN keyboard / mouse / clipboard sharing"
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
    /// Run as a daemon: discover peers, accept connections, sync clipboard.
    Start,
    /// Pair with a new peer (TOFU on next handshake).
    Pair {
        /// Optional peer name hint (advisory).
        #[arg(long)]
        name: Option<String>,
    },
    /// Print local node id, listen address, and known peers.
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let config_dir = resolve_config_dir(&cli)?;
    let cfg = config::load_or_default(&config_dir)?;

    match cli.command {
        Command::Start => {
            let rt = runtime::Runtime::bootstrap(&cfg, &config_dir, /*allow_new_peers*/ false)
                .await
                .context("bootstrap runtime")?;
            rt.run(&cfg).await
        }
        Command::Pair { name: _ } => {
            let rt = runtime::Runtime::bootstrap(&cfg, &config_dir, /*allow_new_peers*/ true)
                .await
                .context("bootstrap pairing runtime")?;
            info!("pairing mode active; press Ctrl-C when done");
            rt.run(&cfg).await
        }
        Command::Status => {
            let identity =
                borderless_transport::Identity::load_or_generate(config_dir.join("identity.key"))?;
            let store = borderless_transport::PeerStore::open(config_dir.join("known_peers.toml"))?;
            println!("node_id   : {}", identity.node_id());
            println!("name      : {}", cfg.node.name);
            println!("bind      : {}", cfg.network.bind_addr());
            println!("config_dir: {}", config_dir.display());
            println!("peers     : {}", store.len());
            for p in store.iter() {
                println!("  - {}  ({})", p.name, &p.pubkey[..16]);
            }
            Ok(())
        }
        Command::Clip(ClipCmd::History { limit }) => {
            // The daemon owns the live history; querying it requires
            // an IPC bridge (planned v0.2). For v0.1 we emit a hint.
            let _ = limit;
            println!(
                "no live IPC yet — run `borderless start` and copy text to populate the daemon's history.\n\
                 v0.2 will expose this over a local UDS bridge."
            );
            Ok(())
        }
        Command::Doctor => {
            let report = doctor::run()?;
            println!("{}", doctor::format(&report));
            Ok(())
        }
    }
}
