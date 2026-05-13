//! Windows `SendInput`-based input injection.

use async_trait::async_trait;
use borderless_core::{Button, InputEvent};
use borderless_pal::{InputEmit, PalError, PalResult};
use std::mem;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_HWHEEL,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
    MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, MOUSE_EVENT_FLAGS, VIRTUAL_KEY, XBUTTON1,
    XBUTTON2,
};

use crate::keymap::hid_to_win;

/// `SendInput` emitter. Cheap to instantiate; the OS owns all state.
pub struct WindowsEmit;

impl WindowsEmit {
    /// New emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsEmit {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl InputEmit for WindowsEmit {
    async fn emit(&mut self, event: InputEvent) -> PalResult<()> {
        match event {
            InputEvent::MouseMove { dx, dy, .. } => {
                let mut input = mouse_input(dx, dy, MOUSEEVENTF_MOVE, 0);
                send(&mut [&mut input])?;
            }
            InputEvent::MouseAbs { x, y, .. } => {
                use windows::Win32::UI::Input::KeyboardAndMouse::{
                    MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_VIRTUALDESK,
                };
                let mut input = mouse_input(
                    x,
                    y,
                    MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                    0,
                );
                send(&mut [&mut input])?;
            }
            InputEvent::MouseButton { btn, pressed } => {
                let (flags, data) = button_flags(btn, pressed);
                let mut input = mouse_input(0, 0, flags, data);
                send(&mut [&mut input])?;
            }
            InputEvent::Scroll { dx, dy } => {
                if dy != 0 {
                    let mut input = mouse_input(0, 0, MOUSEEVENTF_WHEEL, (-dy) * 120);
                    send(&mut [&mut input])?;
                }
                if dx != 0 {
                    let mut input = mouse_input(0, 0, MOUSEEVENTF_HWHEEL, dx * 120);
                    send(&mut [&mut input])?;
                }
            }
            InputEvent::Key { code, pressed, .. } => {
                let Some((vk, scan, ext)) = hid_to_win(code) else {
                    return Ok(());
                };
                let mut flags = KEYEVENTF_SCANCODE;
                if !pressed {
                    flags |= KEYEVENTF_KEYUP;
                }
                if ext {
                    flags |= KEYEVENTF_EXTENDEDKEY;
                }
                let mut input = key_input(vk, scan, flags);
                send(&mut [&mut input])?;
            }
            InputEvent::Enter { .. } | InputEvent::Leave { .. } => {}
        }
        Ok(())
    }
}

fn key_input(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn mouse_input(dx: i32, dy: i32, flags: MOUSE_EVENT_FLAGS, data: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data as u32,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn button_flags(btn: Button, pressed: bool) -> (MOUSE_EVENT_FLAGS, i32) {
    match (btn, pressed) {
        (Button::Left, true) => (MOUSEEVENTF_LEFTDOWN, 0),
        (Button::Left, false) => (MOUSEEVENTF_LEFTUP, 0),
        (Button::Right, true) => (MOUSEEVENTF_RIGHTDOWN, 0),
        (Button::Right, false) => (MOUSEEVENTF_RIGHTUP, 0),
        (Button::Middle, true) => (MOUSEEVENTF_MIDDLEDOWN, 0),
        (Button::Middle, false) => (MOUSEEVENTF_MIDDLEUP, 0),
        (Button::Back, true) => (MOUSEEVENTF_XDOWN, XBUTTON1.0 as i32),
        (Button::Back, false) => (MOUSEEVENTF_XUP, XBUTTON1.0 as i32),
        (Button::Forward, true) => (MOUSEEVENTF_XDOWN, XBUTTON2.0 as i32),
        (Button::Forward, false) => (MOUSEEVENTF_XUP, XBUTTON2.0 as i32),
        (Button::Other(_), _) => (MOUSEEVENTF_MOVE, 0),
    }
}

fn send(inputs: &mut [&mut INPUT]) -> PalResult<()> {
    let inputs_owned: Vec<INPUT> = inputs.iter().map(|i| **i).collect();
    let n = unsafe { SendInput(&inputs_owned, mem::size_of::<INPUT>() as i32) };
    if n == 0 {
        return Err(PalError::Backend(format!(
            "SendInput failed: {:?}",
            unsafe { windows::Win32::Foundation::GetLastError() }
        )));
    }
    Ok(())
}
