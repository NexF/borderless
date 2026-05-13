//! Windows PAL: clipboard backend (always available) plus capture +
//! emit re-exports.

use std::time::Duration;

use async_trait::async_trait;
use borderless_core::{ClipItem, ClipboardSnapshot};
use borderless_pal::{Clipboard, PalError, PalResult};
use tokio::sync::mpsc;

pub use crate::capture::WindowsCapture;
pub use crate::emit::WindowsEmit;

/// arboard-backed clipboard.
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
