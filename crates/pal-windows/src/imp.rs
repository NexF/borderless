use std::time::Duration;

use async_trait::async_trait;
use borderless_core::{ClipItem, ClipboardSnapshot, InputEvent};
use borderless_pal::{
    CaptureMode, Clipboard, EventSink, InputCapture, InputEmit, PalError, PalResult,
};
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// Stub for Windows low-level keyboard + mouse hooks.
pub struct WindowsCapture {
    mode: CaptureMode,
}

impl WindowsCapture {
    /// New capture (no-op until v0.2).
    pub fn new() -> Self {
        Self {
            mode: CaptureMode::Off,
        }
    }
}

impl Default for WindowsCapture {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputCapture for WindowsCapture {
    async fn start(&mut self, _sink: EventSink) -> PalResult<()> {
        warn!("WindowsCapture::start is a v0.1 stub; SetWindowsHookEx wiring lands in v0.2");
        Ok(())
    }
    async fn stop(&mut self) -> PalResult<()> {
        Ok(())
    }
    async fn set_mode(&mut self, mode: CaptureMode) -> PalResult<()> {
        debug!(?mode, "WindowsCapture::set_mode");
        self.mode = mode;
        Ok(())
    }
}

/// Stub for `SendInput`-based emission.
pub struct WindowsEmit;

impl WindowsEmit {
    /// New emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsEmit {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputEmit for WindowsEmit {
    async fn emit(&mut self, _event: InputEvent) -> PalResult<()> {
        Err(PalError::Unsupported(
            "Windows input emission lands in v0.2 (SendInput)",
        ))
    }
}

/// arboard-backed clipboard, identical strategy as the X11 backend.
pub struct WindowsClipboard {
    inner: arboard::Clipboard,
}

impl WindowsClipboard {
    /// Open the Windows clipboard via arboard.
    pub fn new() -> PalResult<Self> {
        let inner = arboard::Clipboard::new().map_err(|e| PalError::Backend(e.to_string()))?;
        Ok(Self { inner })
    }
}

#[async_trait]
impl Clipboard for WindowsClipboard {
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
            self.inner
                .set_text(text)
                .map_err(|e| PalError::Backend(e.to_string()))?;
        }
        Ok(())
    }

    fn watch(&mut self) -> mpsc::UnboundedReceiver<String> {
        let (tx, rx) = mpsc::unbounded_channel();
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
