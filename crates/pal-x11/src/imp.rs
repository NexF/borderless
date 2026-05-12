use std::time::Duration;

use async_trait::async_trait;
use borderless_core::{ClipItem, ClipboardSnapshot, InputEvent};
use borderless_pal::{CaptureMode, Clipboard, EventSink, InputCapture, InputEmit, PalError, PalResult};
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// XInput2-based input capture (stub: not yet implemented).
///
/// The plan for v0.2 is to use [`x11rb`] with the XInput2 extension to
/// receive raw `XI_RawMotion`, `XI_RawButtonPress` and
/// `XI_RawKeyPress` events without grabbing the pointer. Pointer
/// grabbing flips on only when crossing a screen boundary.
pub struct X11Capture {
    mode: CaptureMode,
}

impl X11Capture {
    /// Construct a new capture backend (no-op until `start`).
    pub fn new() -> Self {
        Self { mode: CaptureMode::Off }
    }
}

impl Default for X11Capture {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputCapture for X11Capture {
    async fn start(&mut self, _sink: EventSink) -> PalResult<()> {
        warn!("X11Capture::start is a v0.1 stub; XInput2 wiring lands in v0.2");
        Ok(())
    }

    async fn stop(&mut self) -> PalResult<()> {
        Ok(())
    }

    async fn set_mode(&mut self, mode: CaptureMode) -> PalResult<()> {
        debug!(?mode, "X11Capture::set_mode");
        self.mode = mode;
        Ok(())
    }
}

/// XTest-based input emitter (stub: not yet implemented).
pub struct X11Emit;

impl X11Emit {
    /// New emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for X11Emit {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputEmit for X11Emit {
    async fn emit(&mut self, _event: InputEvent) -> PalResult<()> {
        Err(PalError::Unsupported(
            "X11 input emission lands in v0.2 (XTest)",
        ))
    }
}

/// arboard-backed clipboard. Working today.
pub struct X11Clipboard {
    inner: arboard::Clipboard,
    last_seen: Option<String>,
}

impl X11Clipboard {
    /// Connect to the X server clipboard. Errors if the binary is
    /// running headless without `DISPLAY`.
    pub fn new() -> PalResult<Self> {
        let inner = arboard::Clipboard::new().map_err(|e| PalError::Backend(e.to_string()))?;
        Ok(Self {
            inner,
            last_seen: None,
        })
    }
}

#[async_trait]
impl Clipboard for X11Clipboard {
    async fn read(&mut self) -> PalResult<Option<String>> {
        match self.inner.get_text() {
            Ok(s) => Ok(Some(s)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(PalError::Backend(e.to_string())),
        }
    }

    async fn write(&mut self, snapshot: &ClipboardSnapshot) -> PalResult<()> {
        if let Some(text) = snapshot.items.iter().find_map(|i| match i {
            ClipItem::Text(t) => Some(t.clone()),
            ClipItem::Html { plain_fallback, .. } => Some(plain_fallback.clone()),
            _ => None,
        }) {
            self.last_seen = Some(text.clone());
            self.inner
                .set_text(text)
                .map_err(|e| PalError::Backend(e.to_string()))?;
        }
        Ok(())
    }

    fn watch(&mut self) -> mpsc::UnboundedReceiver<String> {
        let (tx, rx) = mpsc::unbounded_channel();
        // arboard does not have a native watcher; poll at 4 Hz. This is
        // acceptable for v0.1 — the perceived latency target for text
        // clipboard sync is < 50 ms only over the network, not across
        // the local poll interval. v0.2 will replace this with the
        // CLIPBOARD selection notification via x11rb.
        let baseline = self.inner.get_text().ok();
        std::thread::spawn(move || {
            let mut clip = match arboard::Clipboard::new() {
                Ok(c) => c,
                Err(_) => return,
            };
            let mut last = baseline;
            loop {
                std::thread::sleep(Duration::from_millis(250));
                let cur = clip.get_text().ok();
                if cur != last && cur.is_some() {
                    if tx.send(cur.clone().unwrap()).is_err() {
                        break;
                    }
                    last = cur;
                }
            }
        });
        rx
    }
}
