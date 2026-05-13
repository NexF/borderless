//! `borderless` configuration file (`config.toml`).
//!
//! v0.2 splits the configuration along the `kind = "hub" | "spoke"`
//! axis. The Hub binds a TCP+TLS listener on a port; the Spoke
//! initiates outbound connections to a fixed `server_addr`. mDNS-based
//! auto-discovery has been removed.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

/// Default TCP port for the borderless listener.
pub const DEFAULT_PORT: u16 = 38_437;

/// Top-level config.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    /// `[node]` section.
    pub node: NodeConfig,
    /// `[role]` section.
    pub role: RoleConfig,
    /// `[hub]` section (read only when `role.kind == Hub`).
    pub hub: HubConfig,
    /// `[client]` section (read only when `role.kind == Spoke`).
    pub client: ClientConfig,
    /// `[clipboard]` section.
    pub clipboard: ClipboardConfig,
    /// `[input]` section.
    pub input: InputConfig,
}

/// `[node]`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct NodeConfig {
    /// Display name advertised in the SignedHello frame.
    pub name: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            name: gethostname_or("borderless"),
        }
    }
}

/// Role: hub (server) or spoke (client). New installs default to
/// `Unconfigured` so the user is prompted to pick a role on first run.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RoleKind {
    /// Unset; CLI subcommands `serve` / `connect` set this on first
    /// successful run.
    #[default]
    Unconfigured,
    /// This node binds a listener and accepts spokes.
    Hub,
    /// This node dials a hub.
    Spoke,
}

/// `[role]`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RoleConfig {
    /// Persistent role kind.
    pub kind: RoleKind,
}

/// `[hub]` section.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct HubConfig {
    /// TCP socket to bind. `0.0.0.0` for all interfaces.
    pub bind_ip: IpAddr,
    /// Listening port.
    pub port: u16,
    /// Whether unknown spokes may pair (TOFU). Mirrors the v0.1
    /// `pair` mode.
    pub accept_new_peers: bool,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: DEFAULT_PORT,
            accept_new_peers: false,
        }
    }
}

impl HubConfig {
    /// Resolved bind address.
    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new(self.bind_ip, self.port)
    }
}

/// `[client]` section.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ClientConfig {
    /// `host:port` of the hub. Persisted by `borderless connect`.
    pub server_addr: Option<String>,
    /// Optional NodeId pinning. Hex-encoded BLAKE3-truncated pubkey.
    /// When set, the client will refuse to talk to a hub whose
    /// fingerprint doesn't match.
    pub expected_server_id: Option<String>,
}

/// `[clipboard]`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ClipboardConfig {
    /// Number of past snapshots to keep.
    pub history_size: usize,
    /// Whether to sync text.
    pub sync_text: bool,
    /// Whether to sync images. v0.2 supports PNG and JPEG with lazy
    /// fetch for items above the inline threshold.
    pub sync_image: bool,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            history_size: 50,
            sync_text: true,
            sync_image: true,
        }
    }
}

/// `[input]`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct InputConfig {
    /// Spoke-side gate: when false, the spoke ignores `WireFrame::Input`
    /// frames and runs as a "clipboard-only" client. Has no effect on
    /// the hub side.
    pub enabled: bool,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn gethostname_or(default: &str) -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Where the config and state live by default.
pub fn default_config_dir() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("io", "borderless", "borderless")
        .context("could not determine config directory")?;
    Ok(dirs.config_dir().to_path_buf())
}

/// Load `config.toml` from `dir`. If missing, returns the default and
/// writes it back so users have something to edit.
pub fn load_or_default(dir: &Path) -> Result<Config> {
    let path = dir.join("config.toml");
    if path.exists() {
        let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        Ok(toml::from_str(&raw)?)
    } else {
        fs::create_dir_all(dir)?;
        let cfg = Config::default();
        save(dir, &cfg)?;
        Ok(cfg)
    }
}

/// Persist `cfg` back to `dir/config.toml`.
pub fn save(dir: &Path, cfg: &Config) -> Result<()> {
    fs::create_dir_all(dir)?;
    let raw = toml::to_string_pretty(cfg)?;
    fs::write(dir.join("config.toml"), raw)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_are_sensible() {
        let cfg = Config::default();
        assert_eq!(cfg.hub.port, DEFAULT_PORT);
        assert!(cfg.hub.bind_ip.is_unspecified());
        assert!(!cfg.hub.accept_new_peers);
        assert!(cfg.clipboard.sync_text);
        assert!(cfg.clipboard.sync_image);
        assert!(cfg.input.enabled);
        assert_eq!(cfg.role.kind, RoleKind::Unconfigured);
        assert!(!cfg.node.name.is_empty());
    }

    #[test]
    fn missing_file_writes_default_and_returns_it() {
        let dir = tempdir().unwrap();
        let _cfg = load_or_default(dir.path()).unwrap();
        let on_disk = dir.path().join("config.toml");
        assert!(on_disk.exists(), "default file must be written");
    }

    #[test]
    fn user_override_round_trips() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            r#"
[node]
name = "alice"

[role]
kind = "hub"

[hub]
bind_ip = "127.0.0.1"
port = 12345
accept_new_peers = true

[client]
server_addr = "10.0.0.1:9999"

[clipboard]
history_size = 7
sync_text = false
sync_image = false

[input]
enabled = false
"#,
        )
        .unwrap();
        let cfg = load_or_default(dir.path()).unwrap();
        assert_eq!(cfg.node.name, "alice");
        assert_eq!(cfg.role.kind, RoleKind::Hub);
        assert_eq!(cfg.hub.port, 12345);
        assert!(cfg.hub.accept_new_peers);
        assert_eq!(cfg.client.server_addr.as_deref(), Some("10.0.0.1:9999"));
        assert_eq!(cfg.clipboard.history_size, 7);
        assert!(!cfg.clipboard.sync_text);
        assert!(!cfg.clipboard.sync_image);
        assert!(!cfg.input.enabled);
    }

    #[test]
    fn save_and_reload_round_trips() {
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.node.name = "bob".into();
        cfg.role.kind = RoleKind::Spoke;
        cfg.client.server_addr = Some("hub.lan:38437".into());
        save(dir.path(), &cfg).unwrap();

        let loaded = load_or_default(dir.path()).unwrap();
        assert_eq!(loaded.node.name, "bob");
        assert_eq!(loaded.role.kind, RoleKind::Spoke);
        assert_eq!(loaded.client.server_addr.as_deref(), Some("hub.lan:38437"));
    }

    #[test]
    fn malformed_toml_returns_error() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "this is not toml = =").unwrap();
        assert!(load_or_default(dir.path()).is_err());
    }
}
