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
    // Letters (alphabetic).
    /// `a`/`A`.
    pub const KEY_A: Self = Self(0x04);
    /// `b`/`B`.
    pub const KEY_B: Self = Self(0x05);
    /// `c`/`C`.
    pub const KEY_C: Self = Self(0x06);
    /// `d`/`D`.
    pub const KEY_D: Self = Self(0x07);
    /// `e`/`E`.
    pub const KEY_E: Self = Self(0x08);
    /// `f`/`F`.
    pub const KEY_F: Self = Self(0x09);
    /// `g`/`G`.
    pub const KEY_G: Self = Self(0x0A);
    /// `h`/`H`.
    pub const KEY_H: Self = Self(0x0B);
    /// `i`/`I`.
    pub const KEY_I: Self = Self(0x0C);
    /// `j`/`J`.
    pub const KEY_J: Self = Self(0x0D);
    /// `k`/`K`.
    pub const KEY_K: Self = Self(0x0E);
    /// `l`/`L`.
    pub const KEY_L: Self = Self(0x0F);
    /// `m`/`M`.
    pub const KEY_M: Self = Self(0x10);
    /// `n`/`N`.
    pub const KEY_N: Self = Self(0x11);
    /// `o`/`O`.
    pub const KEY_O: Self = Self(0x12);
    /// `p`/`P`.
    pub const KEY_P: Self = Self(0x13);
    /// `q`/`Q`.
    pub const KEY_Q: Self = Self(0x14);
    /// `r`/`R`.
    pub const KEY_R: Self = Self(0x15);
    /// `s`/`S`.
    pub const KEY_S: Self = Self(0x16);
    /// `t`/`T`.
    pub const KEY_T: Self = Self(0x17);
    /// `u`/`U`.
    pub const KEY_U: Self = Self(0x18);
    /// `v`/`V`.
    pub const KEY_V: Self = Self(0x19);
    /// `w`/`W`.
    pub const KEY_W: Self = Self(0x1A);
    /// `x`/`X`.
    pub const KEY_X: Self = Self(0x1B);
    /// `y`/`Y`.
    pub const KEY_Y: Self = Self(0x1C);
    /// `z`/`Z`.
    pub const KEY_Z: Self = Self(0x1D);

    // Digit row.
    /// `1`/`!`.
    pub const KEY_1: Self = Self(0x1E);
    /// `2`/`@`.
    pub const KEY_2: Self = Self(0x1F);
    /// `3`/`#`.
    pub const KEY_3: Self = Self(0x20);
    /// `4`/`$`.
    pub const KEY_4: Self = Self(0x21);
    /// `5`/`%`.
    pub const KEY_5: Self = Self(0x22);
    /// `6`/`^`.
    pub const KEY_6: Self = Self(0x23);
    /// `7`/`&`.
    pub const KEY_7: Self = Self(0x24);
    /// `8`/`*`.
    pub const KEY_8: Self = Self(0x25);
    /// `9`/`(`.
    pub const KEY_9: Self = Self(0x26);
    /// `0`/`)`.
    pub const KEY_0: Self = Self(0x27);

    // Editing & whitespace.
    /// `Enter` / `Return`.
    pub const ENTER: Self = Self(0x28);
    /// `Esc`.
    pub const ESCAPE: Self = Self(0x29);
    /// `Backspace`.
    pub const BACKSPACE: Self = Self(0x2A);
    /// `Tab`.
    pub const TAB: Self = Self(0x2B);
    /// Space.
    pub const SPACE: Self = Self(0x2C);

    // ASCII punctuation row.
    /// `-`/`_`.
    pub const MINUS: Self = Self(0x2D);
    /// `=`/`+`.
    pub const EQUAL: Self = Self(0x2E);
    /// `[`/`{`.
    pub const LEFT_BRACKET: Self = Self(0x2F);
    /// `]`/`}`.
    pub const RIGHT_BRACKET: Self = Self(0x30);
    /// `\\`/`|`.
    pub const BACKSLASH: Self = Self(0x31);
    /// Non-US `#`/`~`.
    pub const NON_US_HASH: Self = Self(0x32);
    /// `;`/`:`.
    pub const SEMICOLON: Self = Self(0x33);
    /// `'`/`"`.
    pub const APOSTROPHE: Self = Self(0x34);
    /// `` ` ``/`~`.
    pub const GRAVE: Self = Self(0x35);
    /// `,`/`<`.
    pub const COMMA: Self = Self(0x36);
    /// `.`/`>`.
    pub const PERIOD: Self = Self(0x37);
    /// `/`/`?`.
    pub const SLASH: Self = Self(0x38);

    // Locks & system.
    /// Caps Lock.
    pub const CAPS_LOCK: Self = Self(0x39);

    // F-row 1..12.
    /// F1.
    pub const F1: Self = Self(0x3A);
    /// F2.
    pub const F2: Self = Self(0x3B);
    /// F3.
    pub const F3: Self = Self(0x3C);
    /// F4.
    pub const F4: Self = Self(0x3D);
    /// F5.
    pub const F5: Self = Self(0x3E);
    /// F6.
    pub const F6: Self = Self(0x3F);
    /// F7.
    pub const F7: Self = Self(0x40);
    /// F8.
    pub const F8: Self = Self(0x41);
    /// F9.
    pub const F9: Self = Self(0x42);
    /// F10.
    pub const F10: Self = Self(0x43);
    /// F11.
    pub const F11: Self = Self(0x44);
    /// F12.
    pub const F12: Self = Self(0x45);

    /// PrintScreen / SysRq.
    pub const PRINT_SCREEN: Self = Self(0x46);
    /// Scroll Lock.
    pub const SCROLL_LOCK: Self = Self(0x47);
    /// Pause / Break.
    pub const PAUSE: Self = Self(0x48);

    // Editing cluster.
    /// Insert.
    pub const INSERT: Self = Self(0x49);
    /// Home.
    pub const HOME: Self = Self(0x4A);
    /// Page Up.
    pub const PAGE_UP: Self = Self(0x4B);
    /// Delete (forward).
    pub const DELETE: Self = Self(0x4C);
    /// End.
    pub const END: Self = Self(0x4D);
    /// Page Down.
    pub const PAGE_DOWN: Self = Self(0x4E);

    // Arrow cluster.
    /// Right Arrow.
    pub const RIGHT: Self = Self(0x4F);
    /// Left Arrow.
    pub const LEFT: Self = Self(0x50);
    /// Down Arrow.
    pub const DOWN: Self = Self(0x51);
    /// Up Arrow.
    pub const UP: Self = Self(0x52);

    // Numeric keypad.
    /// Num Lock / Clear.
    pub const NUM_LOCK: Self = Self(0x53);
    /// Keypad `/`.
    pub const KP_SLASH: Self = Self(0x54);
    /// Keypad `*`.
    pub const KP_ASTERISK: Self = Self(0x55);
    /// Keypad `-`.
    pub const KP_MINUS: Self = Self(0x56);
    /// Keypad `+`.
    pub const KP_PLUS: Self = Self(0x57);
    /// Keypad Enter.
    pub const KP_ENTER: Self = Self(0x58);
    /// Keypad 1 / End.
    pub const KP_1: Self = Self(0x59);
    /// Keypad 2 / Down.
    pub const KP_2: Self = Self(0x5A);
    /// Keypad 3 / PgDn.
    pub const KP_3: Self = Self(0x5B);
    /// Keypad 4 / Left.
    pub const KP_4: Self = Self(0x5C);
    /// Keypad 5.
    pub const KP_5: Self = Self(0x5D);
    /// Keypad 6 / Right.
    pub const KP_6: Self = Self(0x5E);
    /// Keypad 7 / Home.
    pub const KP_7: Self = Self(0x5F);
    /// Keypad 8 / Up.
    pub const KP_8: Self = Self(0x60);
    /// Keypad 9 / PgUp.
    pub const KP_9: Self = Self(0x61);
    /// Keypad 0 / Insert.
    pub const KP_0: Self = Self(0x62);
    /// Keypad `.` / Delete.
    pub const KP_PERIOD: Self = Self(0x63);
    /// Non-US `\\`/`|`.
    pub const NON_US_BACKSLASH: Self = Self(0x64);

    /// Application key (right-click context menu).
    pub const APPLICATION: Self = Self(0x65);

    // Extended F-row (some keyboards have them).
    /// F13.
    pub const F13: Self = Self(0x68);
    /// F14.
    pub const F14: Self = Self(0x69);
    /// F15.
    pub const F15: Self = Self(0x6A);
    /// F16.
    pub const F16: Self = Self(0x6B);
    /// F17.
    pub const F17: Self = Self(0x6C);
    /// F18.
    pub const F18: Self = Self(0x6D);
    /// F19.
    pub const F19: Self = Self(0x6E);
    /// F20.
    pub const F20: Self = Self(0x6F);
    /// F21.
    pub const F21: Self = Self(0x70);
    /// F22.
    pub const F22: Self = Self(0x71);
    /// F23.
    pub const F23: Self = Self(0x72);
    /// F24.
    pub const F24: Self = Self(0x73);

    // Modifier keys (Keyboard page reserves 0xE0..0xE7 for these).
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

    // Selected Consumer-page (0x0C) usages, encoded as 16-bit values
    // with the Consumer-page bit set so PAL backends can disambiguate.
    // These intentionally land outside the Keyboard page's reserved
    // 0x00..0xFF window.
    /// Consumer: Mute.
    pub const CONSUMER_MUTE: Self = Self(0x0C_E2);
    /// Consumer: Volume Up.
    pub const CONSUMER_VOLUME_UP: Self = Self(0x0C_E9);
    /// Consumer: Volume Down.
    pub const CONSUMER_VOLUME_DOWN: Self = Self(0x0C_EA);
    /// Consumer: Play/Pause.
    pub const CONSUMER_PLAY_PAUSE: Self = Self(0x0C_CD);
    /// Consumer: Next Track.
    pub const CONSUMER_NEXT_TRACK: Self = Self(0x0C_B5);
    /// Consumer: Previous Track.
    pub const CONSUMER_PREV_TRACK: Self = Self(0x0C_B6);
    /// Consumer: Stop.
    pub const CONSUMER_STOP: Self = Self(0x0C_B7);
    /// Consumer: Brightness Up.
    pub const CONSUMER_BRIGHTNESS_UP: Self = Self(0x0C_6F);
    /// Consumer: Brightness Down.
    pub const CONSUMER_BRIGHTNESS_DOWN: Self = Self(0x0C_70);
    /// Consumer: Eject.
    pub const CONSUMER_EJECT: Self = Self(0x0C_B8);

    /// True if this usage is on the Consumer page (0x0C).
    pub fn is_consumer(self) -> bool {
        (self.0 & 0xFF00) == 0x0C00
    }

    /// True if this usage is on the Keyboard/Keypad page (0x07).
    pub fn is_keyboard(self) -> bool {
        self.0 <= 0xFF
    }
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
