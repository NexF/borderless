//! `borderless` configuration file (`config.toml`).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

/// Top-level config.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    /// `[node]` section.
    pub node: NodeConfig,
    /// `[network]` section.
    pub network: NetworkConfig,
    /// `[clipboard]` section.
    pub clipboard: ClipboardConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node: NodeConfig::default(),
            network: NetworkConfig::default(),
            clipboard: ClipboardConfig::default(),
        }
    }
}

/// `[node]`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct NodeConfig {
    /// Display name advertised over mDNS / in `Hello`.
    pub name: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            name: gethostname_or("borderless"),
        }
    }
}

/// `[network]`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct NetworkConfig {
    /// UDP port to bind. `0` for ephemeral.
    pub port: u16,
    /// IP to bind. `0.0.0.0` for "all interfaces".
    pub bind_ip: IpAddr,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            port: 38_437,
            bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        }
    }
}

impl NetworkConfig {
    /// Resolved bind address.
    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new(self.bind_ip, self.port)
    }
}

/// `[clipboard]`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ClipboardConfig {
    /// Number of past snapshots to keep.
    pub history_size: usize,
    /// Whether to sync text. v0.1 always honours this.
    pub sync_text: bool,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            history_size: 50,
            sync_text: true,
        }
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
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        Ok(toml::from_str(&raw)?)
    } else {
        fs::create_dir_all(dir)?;
        let cfg = Config::default();
        let raw = toml::to_string_pretty(&cfg)?;
        fs::write(&path, raw)?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_are_sensible() {
        let cfg = Config::default();
        assert_eq!(cfg.network.port, 38_437);
        assert!(cfg.network.bind_ip.is_unspecified());
        assert!(cfg.clipboard.sync_text);
        assert!(cfg.clipboard.history_size > 0);
        assert!(!cfg.node.name.is_empty());
    }

    #[test]
    fn missing_file_writes_default_and_returns_it() {
        let dir = tempdir().unwrap();
        let cfg = load_or_default(dir.path()).unwrap();
        assert_eq!(cfg.network.port, 38_437);
        let on_disk = dir.path().join("config.toml");
        assert!(on_disk.exists(), "default file must be written");
        // Re-reading without modification yields the same values.
        let again = load_or_default(dir.path()).unwrap();
        assert_eq!(again.network.port, cfg.network.port);
        assert_eq!(again.node.name, cfg.node.name);
    }

    #[test]
    fn user_override_round_trips() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            r#"
[node]
name = "alice"

[network]
port = 12345
bind_ip = "127.0.0.1"

[clipboard]
history_size = 7
sync_text = false
"#,
        )
        .unwrap();
        let cfg = load_or_default(dir.path()).unwrap();
        assert_eq!(cfg.node.name, "alice");
        assert_eq!(cfg.network.port, 12345);
        assert_eq!(cfg.network.bind_addr().to_string(), "127.0.0.1:12345");
        assert_eq!(cfg.clipboard.history_size, 7);
        assert!(!cfg.clipboard.sync_text);
    }

    #[test]
    fn malformed_toml_returns_error() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "this is not toml = =").unwrap();
        assert!(load_or_default(dir.path()).is_err());
    }
}
