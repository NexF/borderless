//! XTest-based input emission.
//!
//! Strategy:
//!
//! - Open a dedicated X11 connection, query the XTest extension.
//! - Build a HID keysym → keycode lookup at start; refresh it on
//!   `MappingNotify` events (left as v0.3 polish).
//! - For each [`InputEvent`], synthesize the matching XTest events.
//! - Mouse motion is delta-based via `XTestFakeRelativeMotionEvent`
//!   (XTest extension call).

use async_trait::async_trait;
use borderless_core::{Button, HidUsage, InputEvent};
use borderless_pal::{InputEmit, PalError, PalResult};
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::{debug, warn};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ConnectionExt as _, BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT, KEY_PRESS_EVENT,
    KEY_RELEASE_EVENT, MOTION_NOTIFY_EVENT,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use crate::keymap::{hid_to_keysym, Keysym};

/// XTest emitter.
pub struct X11Emit {
    conn: RustConnection,
    root: u32,
    keysym_to_keycode: Mutex<HashMap<Keysym, u8>>,
}

impl X11Emit {
    /// Open the X server and prepare an XTest emitter.
    pub fn new() -> PalResult<Self> {
        let (conn, screen_num) = RustConnection::connect(None)
            .map_err(|e| PalError::Backend(format!("x11 connect: {e}")))?;
        let setup = conn.setup();
        let root = setup
            .roots
            .get(screen_num)
            .ok_or_else(|| PalError::Backend("no root screen".into()))?
            .root;

        let _ver = conn
            .xtest_get_version(2, 2)
            .map_err(|e| PalError::Backend(format!("xtest version: {e}")))?
            .reply()
            .map_err(|e| PalError::Backend(format!("xtest reply: {e}")))?;

        let map = build_keysym_table(&conn, setup.min_keycode, setup.max_keycode)?;
        Ok(Self {
            conn,
            root,
            keysym_to_keycode: Mutex::new(map),
        })
    }

    /// Translate a HID usage to a keycode known to the X server.
    fn hid_to_keycode(&self, hid: HidUsage) -> Option<u8> {
        let sym = hid_to_keysym(hid)?;
        self.keysym_to_keycode.lock().ok()?.get(&sym).copied()
    }
}

impl Default for X11Emit {
    fn default() -> Self {
        Self::new().expect("open X11 emitter")
    }
}

#[async_trait]
impl InputEmit for X11Emit {
    async fn emit(&mut self, event: InputEvent) -> PalResult<()> {
        match event {
            InputEvent::MouseMove { dx, dy, .. } => {
                self.conn
                    .xtest_fake_input(
                        MOTION_NOTIFY_EVENT,
                        /*relative*/ 1,
                        /*time*/ 0,
                        /*root*/ self.root,
                        dx as i16,
                        dy as i16,
                        /*deviceid*/ 0,
                    )
                    .map_err(|e| PalError::Backend(format!("xtest motion: {e}")))?;
                self.conn
                    .flush()
                    .map_err(|e| PalError::Backend(format!("flush: {e}")))?;
                Ok(())
            }
            InputEvent::MouseAbs { x, y, .. } => {
                self.conn
                    .xtest_fake_input(
                        MOTION_NOTIFY_EVENT,
                        /*relative*/ 0,
                        0,
                        self.root,
                        x as i16,
                        y as i16,
                        0,
                    )
                    .map_err(|e| PalError::Backend(format!("xtest abs motion: {e}")))?;
                self.conn
                    .flush()
                    .map_err(|e| PalError::Backend(format!("flush: {e}")))?;
                Ok(())
            }
            InputEvent::MouseButton { btn, pressed } => {
                let code = button_to_code(btn);
                let ev_type = if pressed {
                    BUTTON_PRESS_EVENT
                } else {
                    BUTTON_RELEASE_EVENT
                };
                self.conn
                    .xtest_fake_input(ev_type, code, 0, self.root, 0, 0, 0)
                    .map_err(|e| PalError::Backend(format!("xtest button: {e}")))?;
                self.conn
                    .flush()
                    .map_err(|e| PalError::Backend(format!("flush: {e}")))?;
                Ok(())
            }
            InputEvent::Scroll { dx, dy } => {
                // Convert to wheel button presses. Each unit = one
                // press+release of buttons 4/5 (vertical) or 6/7
                // (horizontal).
                if dy != 0 {
                    let code = if dy < 0 { 4 } else { 5 };
                    let times = dy.unsigned_abs();
                    for _ in 0..times {
                        self.click(code)?;
                    }
                }
                if dx != 0 {
                    let code = if dx < 0 { 6 } else { 7 };
                    let times = dx.unsigned_abs();
                    for _ in 0..times {
                        self.click(code)?;
                    }
                }
                Ok(())
            }
            InputEvent::Key { code, pressed, .. } => {
                let Some(keycode) = self.hid_to_keycode(code) else {
                    debug!(?code, "no keycode mapping; dropping key");
                    return Ok(());
                };
                let ev_type = if pressed {
                    KEY_PRESS_EVENT
                } else {
                    KEY_RELEASE_EVENT
                };
                self.conn
                    .xtest_fake_input(ev_type, keycode, 0, self.root, 0, 0, 0)
                    .map_err(|e| PalError::Backend(format!("xtest key: {e}")))?;
                self.conn
                    .flush()
                    .map_err(|e| PalError::Backend(format!("flush: {e}")))?;
                Ok(())
            }
            InputEvent::Enter { .. } | InputEvent::Leave { .. } => {
                // Boundary events are router-only; nothing to do at
                // this layer.
                Ok(())
            }
        }
    }
}

impl X11Emit {
    fn click(&self, code: u8) -> PalResult<()> {
        self.conn
            .xtest_fake_input(BUTTON_PRESS_EVENT, code, 0, self.root, 0, 0, 0)
            .map_err(|e| PalError::Backend(format!("xtest scroll press: {e}")))?;
        self.conn
            .xtest_fake_input(BUTTON_RELEASE_EVENT, code, 0, self.root, 0, 0, 0)
            .map_err(|e| PalError::Backend(format!("xtest scroll release: {e}")))?;
        self.conn
            .flush()
            .map_err(|e| PalError::Backend(format!("flush: {e}")))?;
        Ok(())
    }
}

fn button_to_code(btn: Button) -> u8 {
    match btn {
        Button::Left => 1,
        Button::Middle => 2,
        Button::Right => 3,
        Button::Back => 8,
        Button::Forward => 9,
        Button::Other(n) => n,
    }
}

fn build_keysym_table(
    conn: &RustConnection,
    min_kc: u8,
    max_kc: u8,
) -> PalResult<HashMap<Keysym, u8>> {
    let count = max_kc - min_kc + 1;
    let map = conn
        .get_keyboard_mapping(min_kc, count)
        .map_err(|e| PalError::Backend(format!("get_keyboard_mapping: {e}")))?
        .reply()
        .map_err(|e| PalError::Backend(format!("keyboard_mapping reply: {e}")))?;
    let kpkc = map.keysyms_per_keycode as usize;

    let mut out: HashMap<Keysym, u8> = HashMap::new();
    for kc in 0..count {
        let base = (kc as usize) * kpkc;
        for col in 0..kpkc {
            if let Some(&sym) = map.keysyms.get(base + col) {
                if sym == 0 {
                    continue;
                }
                // First-wins: lower-column (unshifted) keysyms are
                // preferred, matching how XKB usually assigns them.
                out.entry(sym).or_insert(min_kc + kc);
            }
        }
    }
    if out.is_empty() {
        warn!("keysym table is empty; XTest emit will reject every key");
    }
    Ok(out)
}

// Suppress unused-import warning if all paths happen to not exercise
// the type.
#[allow(dead_code)]
const _XT_EXT: &str = stringify!(xtest);
