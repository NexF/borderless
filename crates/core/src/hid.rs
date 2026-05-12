//! USB HID Usage codes and modifier bitmask, the lingua-franca for
//! cross-platform key event mapping.
//!
//! Each platform PAL is responsible for translating native key codes
//! (Win VK / macOS keycode / X11 keycode / evdev) to and from this
//! representation. Picking HID Usage Codes as the canonical wire form
//! avoids a tangled web of platform-specific keycode tables.
//!
//! Reference: [USB HID Usage Tables](https://usb.org/sites/default/files/hut1_4.pdf),
//! Section 10 ("Keyboard/Keypad Page (0x07)").

use serde::{Deserialize, Serialize};

/// A USB HID Usage code on the Keyboard/Keypad page (0x07).
///
/// Stored as the 16-bit usage value; the page is implicit (always 0x07
/// in v0). This intentionally allows arbitrary values rather than a
/// closed enum so unknown / vendor keys round-trip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HidUsage(pub u16);

impl HidUsage {
    /// `a` and `A`.
    pub const KEY_A: Self = Self(0x04);
    /// `b` and `B`.
    pub const KEY_B: Self = Self(0x05);
    /// `Enter` / `Return`.
    pub const ENTER: Self = Self(0x28);
    /// `Esc`.
    pub const ESCAPE: Self = Self(0x29);
    /// `Tab`.
    pub const TAB: Self = Self(0x2B);
    /// Space.
    pub const SPACE: Self = Self(0x2C);
    /// Left Control.
    pub const LCTRL: Self = Self(0xE0);
    /// Left Shift.
    pub const LSHIFT: Self = Self(0xE1);
    /// Left Alt / Option.
    pub const LALT: Self = Self(0xE2);
    /// Left GUI (Win / Cmd).
    pub const LGUI: Self = Self(0xE3);
    /// Right Control.
    pub const RCTRL: Self = Self(0xE4);
    /// Right Shift.
    pub const RSHIFT: Self = Self(0xE5);
    /// Right Alt.
    pub const RALT: Self = Self(0xE6);
    /// Right GUI.
    pub const RGUI: Self = Self(0xE7);
}

bitflags::bitflags! {
    /// Bitmask of currently held modifier keys, in HID order.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct ModifierMask: u8 {
        /// Left Control.
        const LCTRL  = 0b0000_0001;
        /// Left Shift.
        const LSHIFT = 0b0000_0010;
        /// Left Alt / Option.
        const LALT   = 0b0000_0100;
        /// Left GUI (Win / Cmd).
        const LGUI   = 0b0000_1000;
        /// Right Control.
        const RCTRL  = 0b0001_0000;
        /// Right Shift.
        const RSHIFT = 0b0010_0000;
        /// Right Alt.
        const RALT   = 0b0100_0000;
        /// Right GUI.
        const RGUI   = 0b1000_0000;
    }
}
