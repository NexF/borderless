//! Platform-specific permission/environment checks.
//!
//! In v0.2 these are role-aware: Hub-side checks bind reachability and
//! capture permissions; Spoke-side checks ability to reach the hub
//! and inject input.

use crate::config::{Config, RoleKind};
use anyhow::Result;
use std::fmt::Write;
use std::path::Path;

/// Result of a single check.
#[derive(Clone, Debug)]
pub struct CheckResult {
    /// Short label.
    pub name: &'static str,
    /// Pass / warn / fail.
    pub status: Status,
    /// Long-form explanation, with hints for fixing.
    pub detail: String,
}

/// One of three possible outcomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Looks good.
    Ok,
    /// Probably fine, but the user should know.
    Warn,
    /// Will not work as-is.
    Fail,
}

impl Status {
    fn glyph(self) -> &'static str {
        match self {
            Status::Ok => " OK ",
            Status::Warn => "WARN",
            Status::Fail => "FAIL",
        }
    }
}

/// Run all the checks suitable for the host platform AND configured role.
pub fn run(cfg: &Config, _config_dir: &Path) -> Result<Vec<CheckResult>> {
    let mut out = Vec::new();
    out.push(check_binary());
    #[cfg(target_os = "linux")]
    out.extend(linux::checks());
    #[cfg(target_os = "macos")]
    out.extend(macos::checks());
    #[cfg(target_os = "windows")]
    out.extend(windows::checks());

    match cfg.role.kind {
        RoleKind::Hub => {
            out.push(check_hub_bind(cfg));
            out.push(check_firewall_hint());
        }
        RoleKind::Spoke => {
            out.push(check_spoke_addr(cfg));
            out.push(check_spoke_reachable(cfg));
        }
        RoleKind::Unconfigured => {
            out.push(CheckResult {
                name: "role",
                status: Status::Warn,
                detail: "no role configured. Run `borderless serve` (Hub) or \
                         `borderless connect <host:port>` (Spoke) once to set one."
                    .into(),
            });
        }
    }

    Ok(out)
}

/// Render the report to a human-readable string.
pub fn format(report: &[CheckResult]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "borderless doctor:\n");
    for c in report {
        let _ = writeln!(s, "  [{}] {}", c.status.glyph(), c.name);
        for line in c.detail.lines() {
            let _ = writeln!(s, "         {}", line);
        }
    }
    s
}

fn check_binary() -> CheckResult {
    CheckResult {
        name: "binary",
        status: Status::Ok,
        detail: format!(
            "borderless v{} on {} ({})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    }
}

fn check_hub_bind(cfg: &Config) -> CheckResult {
    let addr = cfg.hub.bind_addr();
    match std::net::TcpListener::bind(addr) {
        Ok(_) => CheckResult {
            name: "hub bind",
            status: Status::Ok,
            detail: format!("can bind {addr}"),
        },
        Err(e) => CheckResult {
            name: "hub bind",
            status: Status::Fail,
            detail: format!("cannot bind {addr}: {e}"),
        },
    }
}

fn check_firewall_hint() -> CheckResult {
    let port = crate::config::DEFAULT_PORT;
    #[cfg(target_os = "linux")]
    let detail = format!(
        "If spokes can't reach this host, allow inbound TCP {port}.\n\
         e.g.: sudo ufw allow {port}/tcp"
    );
    #[cfg(target_os = "macos")]
    let detail = format!(
        "If spokes can't reach this host, ensure System Settings →\n\
         Network → Firewall allows the borderless binary on TCP {port}."
    );
    #[cfg(target_os = "windows")]
    let detail = format!(
        "If spokes can't reach this host, allow the borderless binary on\n\
         the private network. PowerShell as admin:\n\
         New-NetFirewallRule -DisplayName borderless -Direction Inbound \\\n\
             -Protocol TCP -LocalPort {port} -Action Allow"
    );
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let detail = format!("Allow inbound TCP {port} on this host.");

    CheckResult {
        name: "firewall hint",
        status: Status::Warn,
        detail,
    }
}

fn check_spoke_addr(cfg: &Config) -> CheckResult {
    match &cfg.client.server_addr {
        Some(addr) => CheckResult {
            name: "spoke server addr",
            status: Status::Ok,
            detail: format!("server_addr = {addr}"),
        },
        None => CheckResult {
            name: "spoke server addr",
            status: Status::Fail,
            detail: "server_addr unset. Run `borderless connect <host:port>` once.".into(),
        },
    }
}

fn check_spoke_reachable(cfg: &Config) -> CheckResult {
    let Some(addr) = cfg.client.server_addr.as_deref() else {
        return CheckResult {
            name: "spoke reachability",
            status: Status::Warn,
            detail: "skipped (no server_addr)".into(),
        };
    };
    let timeout = std::time::Duration::from_secs(5);
    let parsed: Vec<std::net::SocketAddr> = match std::net::ToSocketAddrs::to_socket_addrs(addr) {
        Ok(it) => it.collect(),
        Err(e) => {
            return CheckResult {
                name: "spoke reachability",
                status: Status::Fail,
                detail: format!("resolve {addr} failed: {e}"),
            }
        }
    };
    for sa in parsed {
        if std::net::TcpStream::connect_timeout(&sa, timeout).is_ok() {
            return CheckResult {
                name: "spoke reachability",
                status: Status::Ok,
                detail: format!("TCP connect to {sa} succeeded within 5 s"),
            };
        }
    }
    CheckResult {
        name: "spoke reachability",
        status: Status::Fail,
        detail: format!(
            "could not TCP-connect to {addr} within 5 s.\n\
             Check that the hub is running, the firewall allows the port,\n\
             and that the spoke and hub share a routable network."
        ),
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{CheckResult, Status};

    pub fn checks() -> Vec<CheckResult> {
        vec![display_check(), wayland_warning()]
    }

    fn display_check() -> CheckResult {
        match std::env::var("DISPLAY") {
            Ok(d) if !d.is_empty() => CheckResult {
                name: "X11 DISPLAY",
                status: Status::Ok,
                detail: format!("found DISPLAY={d}; clipboard text + X11 input emit should work"),
            },
            _ => CheckResult {
                name: "X11 DISPLAY",
                status: Status::Warn,
                detail: "DISPLAY is unset. Clipboard ops will fail in headless mode.".into(),
            },
        }
    }

    fn wayland_warning() -> CheckResult {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            CheckResult {
                name: "Wayland session",
                status: Status::Warn,
                detail: "WAYLAND_DISPLAY is set. v0.2 only supports the X11/XWayland\n\
                         backend. Native Wayland input lands in v0.3."
                    .into(),
            }
        } else {
            CheckResult {
                name: "Wayland session",
                status: Status::Ok,
                detail: "no WAYLAND_DISPLAY; treating session as X11.".into(),
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{CheckResult, Status};
    pub fn checks() -> Vec<CheckResult> {
        vec![CheckResult {
            name: "Permissions",
            status: Status::Warn,
            detail: "v0.2 input capture/inject requires:\n\
                     - System Settings → Privacy & Security → Accessibility\n\
                     - System Settings → Privacy & Security → Input Monitoring\n\
                     and (for Hub) the binary must be allowed in both."
                .into(),
        }]
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{CheckResult, Status};
    pub fn checks() -> Vec<CheckResult> {
        vec![CheckResult {
            name: "Firewall",
            status: Status::Warn,
            detail: "On first start Windows Defender Firewall will prompt to allow\n\
                     `borderless` on the private network. Allow it for the hub to\n\
                     accept spoke connections, and for spokes to dial out."
                .into(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tempfile::tempdir;

    #[test]
    fn run_includes_binary_check_and_role_check() {
        let cfg = Config::default();
        let dir = tempdir().unwrap();
        let report = run(&cfg, dir.path()).unwrap();
        assert!(!report.is_empty());
        assert!(report.iter().any(|c| c.name == "binary"));
        assert!(report.iter().any(|c| c.name == "role"));
    }

    #[test]
    fn format_includes_status_glyph_and_name() {
        let report = vec![
            CheckResult {
                name: "alpha",
                status: Status::Ok,
                detail: "fine".into(),
            },
            CheckResult {
                name: "beta",
                status: Status::Warn,
                detail: "watch out".into(),
            },
            CheckResult {
                name: "gamma",
                status: Status::Fail,
                detail: "broken".into(),
            },
        ];
        let s = format(&report);
        assert!(s.contains("[ OK ] alpha"));
        assert!(s.contains("[WARN] beta"));
        assert!(s.contains("[FAIL] gamma"));
    }

    #[test]
    fn glyph_widths_are_consistent() {
        assert_eq!(Status::Ok.glyph().chars().count(), 4);
        assert_eq!(Status::Warn.glyph().chars().count(), 4);
        assert_eq!(Status::Fail.glyph().chars().count(), 4);
    }
}
