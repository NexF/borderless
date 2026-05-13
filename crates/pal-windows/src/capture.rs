//! Windows low-level keyboard + mouse hook capture.
//!
//! The hook procedure is required by Windows to be a regular `extern
//! "system" fn`, not a closure, so we forward through a process-wide
//! `OnceCell<UnboundedSender<InputEvent>>`. A dedicated thread sets
//! up the hooks and runs `GetMessage` to keep the hook callbacks
//! alive — Windows only invokes `WH_KEYBOARD_LL` / `WH_MOUSE_LL`
//! callbacks on a thread that's pumping a message loop.

use async_trait::async_trait;
use borderless_core::{Button, HidUsage, InputEvent, ModifierMask};
use borderless_pal::{CaptureMode, EventSink, InputCapture, PalError, PalResult};
use parking_lot::Mutex as ParkingMutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use tracing::{debug, warn};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL,
    WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN,
    WM_XBUTTONUP, XBUTTON1,
};

use crate::keymap::vk_to_hid;

static SINK: OnceLock<EventSink> = OnceLock::new();
static MODIFIERS: OnceLock<ParkingMutex<ModifierMask>> = OnceLock::new();
static LAST_MOUSE: OnceLock<ParkingMutex<Option<(i32, i32)>>> = OnceLock::new();

/// WH_KEYBOARD_LL / WH_MOUSE_LL capture handle.
pub struct WindowsCapture {
    mode: CaptureMode,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl WindowsCapture {
    /// Construct a new capture (idle until `start`).
    pub fn new() -> Self {
        Self {
            mode: CaptureMode::Off,
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
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
    async fn start(&mut self, sink: EventSink) -> PalResult<()> {
        if self.thread.is_some() {
            return Ok(());
        }
        // Process-wide initialisation; only the first start wins.
        let _ = SINK.set(sink);
        let _ = MODIFIERS.set(ParkingMutex::new(ModifierMask::empty()));
        let _ = LAST_MOUSE.set(ParkingMutex::new(None));
        self.stop.store(false, Ordering::SeqCst);
        let stop = self.stop.clone();
        let handle = std::thread::Builder::new()
            .name("borderless-win-capture".into())
            .spawn(move || {
                if let Err(e) = run_hooks(stop) {
                    warn!(error = ?e, "windows capture thread exited");
                }
            })
            .map_err(|e| PalError::Backend(format!("spawn capture thread: {e}")))?;
        self.thread = Some(handle);
        Ok(())
    }

    async fn stop(&mut self) -> PalResult<()> {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.thread.take() {
            std::mem::drop(h);
        }
        Ok(())
    }

    async fn set_mode(&mut self, mode: CaptureMode) -> PalResult<()> {
        debug!(?mode, "WindowsCapture::set_mode");
        self.mode = mode;
        if mode == CaptureMode::Grab {
            warn!("CaptureMode::Grab not yet implemented in v0.2");
        }
        Ok(())
    }
}

fn run_hooks(stop: Arc<AtomicBool>) -> PalResult<()> {
    use windows::Win32::Foundation::HMODULE;
    let kbd = unsafe {
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), HMODULE(0), 0)
            .map_err(|e| PalError::Backend(format!("SetWindowsHookExW kbd: {e}")))?
    };
    let mouse = unsafe {
        SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_proc), HMODULE(0), 0)
            .map_err(|e| PalError::Backend(format!("SetWindowsHookExW mouse: {e}")))?
    };

    // Pump messages so the hook callbacks fire.
    let mut msg = MSG::default();
    while !stop.load(Ordering::Relaxed) {
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if r.0 == 0 || r.0 == -1 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    unsafe {
        let _ = UnhookWindowsHookEx(kbd);
        let _ = UnhookWindowsHookEx(mouse);
    }
    Ok(())
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let kbd = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let pressed = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        let released = matches!(wparam.0 as u32, WM_KEYUP | WM_SYSKEYUP);
        if pressed || released {
            if let Some(hid) = vk_to_hid(kbd.vkCode as u16) {
                let mods = MODIFIERS.get().unwrap();
                {
                    let mut m = mods.lock();
                    update_modifiers(hid, pressed, &mut m);
                }
                if let Some(sink) = SINK.get() {
                    let _ = sink.send(InputEvent::Key {
                        code: hid,
                        pressed,
                        modifiers: *mods.lock(),
                    });
                }
            }
        }
    }
    CallNextHookEx(HHOOK(0), code, wparam, lparam)
}

unsafe extern "system" fn low_level_mouse_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let m = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let sink = SINK.get();
        match wparam.0 as u32 {
            WM_MOUSEMOVE => {
                let last = LAST_MOUSE.get().unwrap();
                let mut g = last.lock();
                let (x, y) = (m.pt.x, m.pt.y);
                if let Some((px, py)) = *g {
                    let dx = x - px;
                    let dy = y - py;
                    if let Some(s) = sink {
                        let _ = s.send(InputEvent::MouseMove {
                            dx,
                            dy,
                            ts: now_ms(),
                        });
                    }
                }
                *g = Some((x, y));
            }
            WM_LBUTTONDOWN => button(sink, Button::Left, true),
            WM_LBUTTONUP => button(sink, Button::Left, false),
            WM_RBUTTONDOWN => button(sink, Button::Right, true),
            WM_RBUTTONUP => button(sink, Button::Right, false),
            WM_MBUTTONDOWN => button(sink, Button::Middle, true),
            WM_MBUTTONUP => button(sink, Button::Middle, false),
            WM_XBUTTONDOWN | WM_XBUTTONUP => {
                let xbtn = (m.mouseData >> 16) as u16;
                let pressed = wparam.0 as u32 == WM_XBUTTONDOWN;
                let b = if xbtn == XBUTTON1.0 {
                    Button::Back
                } else {
                    Button::Forward
                };
                button(sink, b, pressed);
            }
            WM_MOUSEWHEEL => {
                let delta = (m.mouseData >> 16) as i16;
                if let Some(s) = sink {
                    let _ = s.send(InputEvent::Scroll {
                        dx: 0,
                        dy: -(delta / 120) as i32,
                    });
                }
            }
            _ => {}
        }
    }
    CallNextHookEx(HHOOK(0), code, wparam, lparam)
}

fn button(sink: Option<&EventSink>, btn: Button, pressed: bool) {
    if let Some(s) = sink {
        let _ = s.send(InputEvent::MouseButton { btn, pressed });
    }
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

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
