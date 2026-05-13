//! macOS `CGEventPost`-based input injection.

use async_trait::async_trait;
use borderless_core::{Button, InputEvent};
use borderless_pal::{InputEmit, PalError, PalResult};
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use parking_lot::Mutex;

use crate::keymap::hid_to_mac;

/// CGEvent emitter. Owns a `CGEventSource` that tags every synthetic
/// event so the system distinguishes them from physical input.
pub struct MacosEmit {
    source: CGEventSource,
    last_pos: Mutex<CGPoint>,
}

impl MacosEmit {
    /// Build a new emitter.
    pub fn new() -> PalResult<Self> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| PalError::Backend("CGEventSource::new failed".into()))?;
        Ok(Self {
            source,
            last_pos: Mutex::new(CGPoint::new(0.0, 0.0)),
        })
    }
}

#[async_trait]
impl InputEmit for MacosEmit {
    async fn emit(&mut self, event: InputEvent) -> PalResult<()> {
        match event {
            InputEvent::MouseMove { dx, dy, .. } => {
                let mut p = self.last_pos.lock();
                p.x += dx as f64;
                p.y += dy as f64;
                let pos = *p;
                drop(p);
                let ev = CGEvent::new_mouse_event(
                    self.source.clone(),
                    CGEventType::MouseMoved,
                    pos,
                    CGMouseButton::Left,
                )
                .map_err(|_| PalError::Backend("mouse moved event".into()))?;
                ev.post(CGEventTapLocation::HID);
                Ok(())
            }
            InputEvent::MouseAbs { x, y, .. } => {
                let pos = CGPoint::new(x as f64, y as f64);
                *self.last_pos.lock() = pos;
                let ev = CGEvent::new_mouse_event(
                    self.source.clone(),
                    CGEventType::MouseMoved,
                    pos,
                    CGMouseButton::Left,
                )
                .map_err(|_| PalError::Backend("mouse abs event".into()))?;
                ev.post(CGEventTapLocation::HID);
                Ok(())
            }
            InputEvent::MouseButton { btn, pressed } => {
                let pos = *self.last_pos.lock();
                let (et, mb) = mouse_event_type(btn, pressed);
                let ev = CGEvent::new_mouse_event(self.source.clone(), et, pos, mb)
                    .map_err(|_| PalError::Backend("mouse button event".into()))?;
                ev.post(CGEventTapLocation::HID);
                Ok(())
            }
            InputEvent::Scroll { dx, dy } => {
                let ev = CGEvent::new_scroll_event(
                    self.source.clone(),
                    ScrollEventUnit::LINE,
                    2,
                    -dy,
                    -dx,
                    0,
                )
                .map_err(|_| PalError::Backend("scroll event".into()))?;
                ev.post(CGEventTapLocation::HID);
                Ok(())
            }
            InputEvent::Key { code, pressed, .. } => {
                let Some(kc) = hid_to_mac(code) else {
                    return Ok(());
                };
                let ev = CGEvent::new_keyboard_event(self.source.clone(), kc, pressed)
                    .map_err(|_| PalError::Backend("keyboard event".into()))?;
                ev.post(CGEventTapLocation::HID);
                Ok(())
            }
            InputEvent::Enter { .. } | InputEvent::Leave { .. } => Ok(()),
        }
    }
}

fn mouse_event_type(btn: Button, pressed: bool) -> (CGEventType, CGMouseButton) {
    match (btn, pressed) {
        (Button::Left, true) => (CGEventType::LeftMouseDown, CGMouseButton::Left),
        (Button::Left, false) => (CGEventType::LeftMouseUp, CGMouseButton::Left),
        (Button::Right, true) => (CGEventType::RightMouseDown, CGMouseButton::Right),
        (Button::Right, false) => (CGEventType::RightMouseUp, CGMouseButton::Right),
        (Button::Middle, true) => (CGEventType::OtherMouseDown, CGMouseButton::Center),
        (Button::Middle, false) => (CGEventType::OtherMouseUp, CGMouseButton::Center),
        (_, true) => (CGEventType::OtherMouseDown, CGMouseButton::Center),
        (_, false) => (CGEventType::OtherMouseUp, CGMouseButton::Center),
    }
}
