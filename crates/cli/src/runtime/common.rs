//! Shared helpers used by both ServerRuntime and ClientRuntime.

use anyhow::Result;
use borderless_clipboard::{Decision, Engine as ClipboardEngine};
use borderless_core::{ClipboardSnapshot, WireFrame};
use borderless_pal::Clipboard as ClipboardTrait;
use tokio::sync::mpsc;
use tracing::warn;

/// Open the host clipboard backend (or `None` if unsupported).
#[cfg(target_os = "linux")]
pub fn open_local_clipboard() -> Result<Option<Box<dyn ClipboardTrait>>> {
    match borderless_pal_x11::X11Clipboard::new() {
        Ok(c) => Ok(Some(Box::new(c))),
        Err(e) => {
            warn!(error = %e, "X11 clipboard unavailable");
            Ok(None)
        }
    }
}

/// Open the host clipboard backend (or `None` if unsupported).
#[cfg(target_os = "windows")]
pub fn open_local_clipboard() -> Result<Option<Box<dyn ClipboardTrait>>> {
    match borderless_pal_windows::WindowsClipboard::new() {
        Ok(c) => Ok(Some(Box::new(c))),
        Err(e) => {
            warn!(error = %e, "Windows clipboard unavailable");
            Ok(None)
        }
    }
}

/// Open the host clipboard backend (or `None` if unsupported).
#[cfg(target_os = "macos")]
pub fn open_local_clipboard() -> Result<Option<Box<dyn ClipboardTrait>>> {
    match borderless_pal_macos::MacosClipboard::new() {
        Ok(c) => Ok(Some(Box::new(c))),
        Err(e) => {
            warn!(error = %e, "macOS clipboard unavailable");
            Ok(None)
        }
    }
}

/// Open the host clipboard backend (or `None` if unsupported).
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn open_local_clipboard() -> Result<Option<Box<dyn ClipboardTrait>>> {
    Ok(None)
}

/// Apply a clipboard snapshot to the local OS clipboard, opening a
/// fresh backend handle each time so we don't keep a long-lived
/// X11 connection in the runtime.
pub async fn apply_clipboard_snapshot(snap: &ClipboardSnapshot) -> Result<()> {
    if let Some(mut clip) = open_local_clipboard()? {
        clip.write(snap)
            .await
            .map_err(|e| anyhow::anyhow!("write clipboard: {e}"))?;
    }
    Ok(())
}

/// Pure helper: given the clipboard engine and a freshly-received
/// snapshot, decide whether to apply it locally.
pub async fn handle_remote_clipboard(
    clipboard: &ClipboardEngine,
    snap: ClipboardSnapshot,
) -> Option<ClipboardSnapshot> {
    let v = snap.version;
    let origin = snap.origin;
    let preview: String = snap
        .items
        .iter()
        .find_map(|i| {
            if let borderless_core::ClipItem::Text(t) = i {
                Some(t.chars().take(40).collect::<String>())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "(non-text)".to_string());
    match clipboard.observe_remote(snap) {
        Decision::Apply(snap) => {
            tracing::info!(version = v, %origin, preview = %preview, "applied remote clipboard");
            if let Err(e) = apply_clipboard_snapshot(&snap).await {
                warn!(error = %e, "apply clipboard failed");
            }
            Some(snap)
        }
        Decision::Ignore => {
            tracing::debug!(version = v, %origin, "ignored stale/echoed clipboard");
            None
        }
    }
}

/// Pure helper: read the next text from the local clipboard watcher,
/// turn it into a `WireFrame::Clipboard`, and return it ready to send.
pub fn watch_to_frame(clipboard: &ClipboardEngine, text: String) -> WireFrame {
    let snap = clipboard.produce_text(text);
    WireFrame::Clipboard(snap)
}

/// Spawn a background watcher that emits each local clipboard change as
/// a `WireFrame::Clipboard` ready to send. Returns the receiver end
/// the caller forwards to the network.
pub fn spawn_local_clipboard_watcher(
    mut watcher: mpsc::UnboundedReceiver<String>,
    clipboard: ClipboardEngine,
) -> mpsc::UnboundedReceiver<WireFrame> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(text) = watcher.recv().await {
            let frame = watch_to_frame(&clipboard, text);
            if tx.send(frame).is_err() {
                break;
            }
        }
    });
    rx
}
