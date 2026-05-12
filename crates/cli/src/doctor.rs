//! Platform-specific permission/environment checks.

use anyhow::Result;
use std::fmt::Write;

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
    #[allow(dead_code)] // emitted by future platform-specific checks
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

/// Run all the checks suitable for the host platform.
pub fn run() -> Result<Vec<CheckResult>> {
    let mut out = Vec::new();
    out.push(check_rust_version());
    #[cfg(target_os = "linux")]
    out.extend(linux::checks());
    #[cfg(target_os = "macos")]
    out.extend(macos::checks());
    #[cfg(target_os = "windows")]
    out.extend(windows::checks());
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

fn check_rust_version() -> CheckResult {
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

#[cfg(target_os = "linux")]
mod linux {
    use super::{CheckResult, Status};

    pub fn checks() -> Vec<CheckResult> {
        let mut v = Vec::new();
        v.push(display_check());
        v.push(wayland_warning());
        v
    }

    fn display_check() -> CheckResult {
        match std::env::var("DISPLAY") {
            Ok(d) if !d.is_empty() => CheckResult {
                name: "X11 DISPLAY",
                status: Status::Ok,
                detail: format!("found DISPLAY={d}; clipboard via arboard should work"),
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
                detail: "WAYLAND_DISPLAY is set. v0.1 only does clipboard text under XWayland; \
                         input capture/inject lands in v0.3."
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
            detail: "v0.1 ships clipboard only. Real input capture will require\n\
                     System Settings -> Privacy & Security -> Accessibility AND\n\
                     System Settings -> Privacy & Security -> Input Monitoring\n\
                     to allow `borderless`."
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
                     `borderless` on the private network. Allow it for clipboard\n\
                     and (later) input sync to work."
                .into(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_returns_at_least_the_binary_check() {
        let report = run().unwrap();
        assert!(!report.is_empty());
        assert!(report.iter().any(|c| c.name == "binary"));
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
        assert!(s.contains("watch out"));
    }

    #[test]
    fn glyph_widths_are_consistent() {
        // Glyphs all render to 4 chars so the CLI columns line up.
        assert_eq!(Status::Ok.glyph().chars().count(), 4);
        assert_eq!(Status::Warn.glyph().chars().count(), 4);
        assert_eq!(Status::Fail.glyph().chars().count(), 4);
    }
}
