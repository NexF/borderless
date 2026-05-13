//! XInput2-based input capture.
//!
//! Strategy:
//!
//! - Open a *separate* X11 connection (so blocking on events can't
//!   interfere with arboard's clipboard connection).
//! - Query XInput2 (≥ 2.2) on the default screen.
//! - Subscribe to `RawKeyPress`, `RawKeyRelease`, `RawButtonPress`,
//!   `RawButtonRelease`, `RawMotion` on the root window for the
//!   `AllMasterDevices` device id.
//! - Spawn a dedicated `std::thread` that pumps `wait_for_event`.
//!   Decoded events are sent to the [`EventSink`].
//!
//! v0.2 ships **Listen** mode only: events are observed alongside
//! their normal local delivery. `Grab` mode (XIGrabDevice + suppress
//! local delivery while cursor lives on a remote screen) is the v0.3
//! polish item; the runtime currently never asks for it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use async_trait::async_trait;
use borderless_core::{Button, HidUsage, InputEvent, ModifierMask};
use borderless_pal::{CaptureMode, EventSink, InputCapture, PalError, PalResult};
use tracing::{debug, error, info, warn};
use x11rb::connection::Connection;
use x11rb::protocol::xinput::{self, ConnectionExt as _, EventMask, XIEventMask};
use x11rb::protocol::xproto::{ConnectionExt as _, KEY_PRESS_EVENT};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use crate::keymap::keysym_to_hid;

/// XInput2 capture handle.
pub struct X11Capture {
    mode: CaptureMode,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl X11Capture {
    /// Construct a new capture backend (idle until `start`).
    pub fn new() -> Self {
        Self {
            mode: CaptureMode::Off,
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }
}

impl Default for X11Capture {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputCapture for X11Capture {
    async fn start(&mut self, sink: EventSink) -> PalResult<()> {
        if self.thread.is_some() {
            return Ok(());
        }
        // The connection lives entirely inside the worker thread.
        let stop = self.stop.clone();
        self.stop.store(false, Ordering::SeqCst);
        let handle = std::thread::Builder::new()
            .name("borderless-x11-capture".into())
            .spawn(move || {
                if let Err(e) = run_capture_loop(sink, stop) {
                    error!(error = ?e, "x11 capture loop failed");
                }
            })
            .map_err(|e| PalError::Backend(format!("spawn capture thread: {e}")))?;
        self.thread = Some(handle);
        Ok(())
    }

    async fn stop(&mut self) -> PalResult<()> {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.thread.take() {
            // We can't safely join: the worker is parked on
            // wait_for_event. Detach instead. v0.3 will use a self-
            // pipe to wake the loop for clean shutdown.
            std::mem::drop(h);
        }
        Ok(())
    }

    async fn set_mode(&mut self, mode: CaptureMode) -> PalResult<()> {
        debug!(?mode, "X11Capture::set_mode");
        self.mode = mode;
        // Grab mode arrives in v0.3 (XIGrabDevice).
        if mode == CaptureMode::Grab {
            warn!("CaptureMode::Grab not yet implemented in v0.2; running in Listen mode");
        }
        Ok(())
    }
}

fn run_capture_loop(sink: EventSink, stop: Arc<AtomicBool>) -> PalResult<()> {
    let (conn, screen_num) = RustConnection::connect(None)
        .map_err(|e| PalError::Backend(format!("x11 connect: {e}")))?;
    let setup = conn.setup();
    let root = setup
        .roots
        .get(screen_num)
        .ok_or_else(|| PalError::Backend("no root screen".into()))?
        .root;

    // Make sure XInput2 is present.
    let xi_query = conn
        .xinput_xi_query_version(2, 2)
        .map_err(|e| PalError::Backend(format!("xinput query: {e}")))?
        .reply()
        .map_err(|e| PalError::Backend(format!("xinput reply: {e}")))?;
    info!(
        major = xi_query.major_version,
        minor = xi_query.minor_version,
        "xinput2 ready"
    );

    // Subscribe to all raw events on the root window.
    let mask = XIEventMask::RAW_KEY_PRESS
        | XIEventMask::RAW_KEY_RELEASE
        | XIEventMask::RAW_BUTTON_PRESS
        | XIEventMask::RAW_BUTTON_RELEASE
        | XIEventMask::RAW_MOTION;
    let event_mask = EventMask {
        deviceid: xinput::Device::ALL_MASTER.into(),
        mask: vec![mask],
    };
    conn.xinput_xi_select_events(root, &[event_mask])
        .map_err(|e| PalError::Backend(format!("xi_select_events: {e}")))?;
    conn.flush()
        .map_err(|e| PalError::Backend(format!("flush: {e}")))?;

    // Build a keycode -> keysym table once. (No xkb extension; we
    // use the simpler core key mapping.)
    let min_kc = setup.min_keycode;
    let max_kc = setup.max_keycode;
    let count = max_kc - min_kc + 1;
    let key_map = conn
        .get_keyboard_mapping(min_kc, count)
        .map_err(|e| PalError::Backend(format!("get_keyboard_mapping: {e}")))?
        .reply()
        .map_err(|e| PalError::Backend(format!("keyboard_mapping reply: {e}")))?;
    let kpkc = key_map.keysyms_per_keycode as usize;
    let keysyms: Vec<u32> = key_map.keysyms;

    let mut modifiers = ModifierMask::empty();

    while !stop.load(Ordering::Relaxed) {
        let event = match conn.wait_for_event() {
            Ok(e) => e,
            Err(e) => {
                warn!(error = %e, "wait_for_event failed; ending capture loop");
                return Err(PalError::Backend(e.to_string()));
            }
        };
        dispatch_event(&event, &keysyms, kpkc, min_kc, &mut modifiers, &sink);
    }
    Ok(())
}

fn dispatch_event(
    event: &Event,
    keysyms: &[u32],
    kpkc: usize,
    min_kc: u8,
    modifiers: &mut ModifierMask,
    sink: &EventSink,
) {
    match event {
        Event::XinputRawKeyPress(ev) => {
            handle_key(ev.detail, true, keysyms, kpkc, min_kc, modifiers, sink);
        }
        Event::XinputRawKeyRelease(ev) => {
            handle_key(ev.detail, false, keysyms, kpkc, min_kc, modifiers, sink);
        }
        Event::XinputRawButtonPress(ev) => {
            handle_button(ev.detail, true, sink);
        }
        Event::XinputRawButtonRelease(ev) => {
            handle_button(ev.detail, false, sink);
        }
        Event::XinputRawMotion(ev) => {
            // RawMotion delivers values in `axisvalues_raw` aligned
            // with the bits of `valuator_mask`. Axes 0 and 1 are X
            // and Y; values are 32:32 fixed-point Fp3232. We sum the
            // integer halves into a single delta event.
            let mut dx = 0i32;
            let mut dy = 0i32;
            for (axis, val) in mask_iter(&ev.valuator_mask, &ev.axisvalues_raw) {
                if axis == 0 {
                    dx = fp3232_to_i32(val);
                } else if axis == 1 {
                    dy = fp3232_to_i32(val);
                }
            }
            if dx != 0 || dy != 0 {
                let _ = sink.send(InputEvent::MouseMove {
                    dx,
                    dy,
                    ts: now_ms(),
                });
            }
        }
        _ => {
            // Ignore non-XI events.
        }
    }
}

fn handle_key(
    keycode: u32,
    pressed: bool,
    keysyms: &[u32],
    kpkc: usize,
    min_kc: u8,
    modifiers: &mut ModifierMask,
    sink: &EventSink,
) {
    let kc = keycode as i32 - min_kc as i32;
    if kc < 0 {
        return;
    }
    let base_idx = kc as usize * kpkc;
    let Some(&sym) = keysyms.get(base_idx) else {
        return;
    };
    let Some(hid) = keysym_to_hid(sym) else {
        debug!(sym, kc = keycode, "unknown keysym; dropping");
        return;
    };

    update_modifiers(hid, pressed, modifiers);

    let _ = sink.send(InputEvent::Key {
        code: hid,
        pressed,
        modifiers: *modifiers,
    });
}

fn update_modifiers(hid: HidUsage, pressed: bool, m: &mut ModifierMask) {
    let bit = match hid {
        HidUsage::LCTRL => Some(ModifierMask::LCTRL),
        HidUsage::LSHIFT => Some(ModifierMask::LSHIFT),
        HidUsage::LALT => Some(ModifierMask::LALT),
        HidUsage::LGUI => Some(ModifierMask::LGUI),
        HidUsage::RCTRL => Some(ModifierMask::RCTRL),
        HidUsage::RSHIFT => Some(ModifierMask::RSHIFT),
        HidUsage::RALT => Some(ModifierMask::RALT),
        HidUsage::RGUI => Some(ModifierMask::RGUI),
        _ => None,
    };
    if let Some(b) = bit {
        if pressed {
            m.insert(b);
        } else {
            m.remove(b);
        }
    }
}

fn handle_button(detail: u32, pressed: bool, sink: &EventSink) {
    // X11 button numbering: 1 left, 2 middle, 3 right, 4..7 wheel,
    // 8/9 back/forward.
    match detail {
        1 => {
            let _ = sink.send(InputEvent::MouseButton {
                btn: Button::Left,
                pressed,
            });
        }
        2 => {
            let _ = sink.send(InputEvent::MouseButton {
                btn: Button::Middle,
                pressed,
            });
        }
        3 => {
            let _ = sink.send(InputEvent::MouseButton {
                btn: Button::Right,
                pressed,
            });
        }
        4 if pressed => {
            let _ = sink.send(InputEvent::Scroll { dx: 0, dy: -1 });
        }
        5 if pressed => {
            let _ = sink.send(InputEvent::Scroll { dx: 0, dy: 1 });
        }
        6 if pressed => {
            let _ = sink.send(InputEvent::Scroll { dx: -1, dy: 0 });
        }
        7 if pressed => {
            let _ = sink.send(InputEvent::Scroll { dx: 1, dy: 0 });
        }
        8 => {
            let _ = sink.send(InputEvent::MouseButton {
                btn: Button::Back,
                pressed,
            });
        }
        9 => {
            let _ = sink.send(InputEvent::MouseButton {
                btn: Button::Forward,
                pressed,
            });
        }
        b => {
            let _ = sink.send(InputEvent::MouseButton {
                btn: Button::Other(b as u8),
                pressed,
            });
        }
    }
}

fn fp3232_to_i32(v: xinput::Fp3232) -> i32 {
    // Fp3232 = integral:32 + frac:32. Round toward zero.
    v.integral
}

fn mask_iter<'a>(
    mask: &'a [u32],
    values: &'a [xinput::Fp3232],
) -> impl Iterator<Item = (usize, xinput::Fp3232)> + 'a {
    let mut value_idx = 0usize;
    let mut out = Vec::new();
    for (word_idx, &word) in mask.iter().enumerate() {
        for bit in 0..32 {
            if word & (1 << bit) != 0 {
                let axis = word_idx * 32 + bit;
                if let Some(v) = values.get(value_idx) {
                    out.push((axis, *v));
                }
                value_idx += 1;
            }
        }
    }
    out.into_iter()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// Quiet unused-import warning — `KEY_PRESS_EVENT` is used as a sanity
// reminder but not directly referenced.
#[allow(dead_code)]
const _: u8 = KEY_PRESS_EVENT;
