//! Static HID Usage ↔ X11 keysym lookup tables.
//!
//! At runtime the X11 server may map any keysym to any keycode, so we
//! always go via *keysym* and then resolve a keycode dynamically.
//! This file is the only place that hard-codes "what does HID code N
//! mean" for the X11 backend.

use borderless_core::HidUsage;

/// A 32-bit X11 keysym (e.g. `XK_a` is `0x0061`, `XK_Return` is `0xff0d`).
pub type Keysym = u32;

/// HID usage 0x04..=0x1D inclusive: a..z. We emit lowercase keysyms;
/// shift state is tracked separately on the wire (see ModifierMask).
fn alpha_to_keysym(usage: u16) -> Option<Keysym> {
    let n = usage as i32 - 0x04;
    if (0..=25).contains(&n) {
        // X keysym for `a` is 0x0061, sequential alphabet.
        Some(0x0061 + n as u32)
    } else {
        None
    }
}

/// HID usage 0x1E..=0x27 inclusive: 1234567890. We emit the digit
/// keysym (e.g. 0x0031 for '1'); shift transforms server-side.
fn digit_to_keysym(usage: u16) -> Option<Keysym> {
    if (0x1E..=0x27).contains(&usage) {
        // 0x1E -> '1', 0x1F -> '2', ..., 0x26 -> '9', 0x27 -> '0'.
        let digit = if usage == 0x27 {
            0
        } else {
            (usage - 0x1E + 1) as u32
        };
        Some(0x0030 + digit)
    } else {
        None
    }
}

/// Translate a HID usage to its primary X11 keysym, if known.
///
/// Unknown HID codes fall through to `None`; the caller drops them.
pub fn hid_to_keysym(usage: HidUsage) -> Option<Keysym> {
    if let Some(k) = alpha_to_keysym(usage.0) {
        return Some(k);
    }
    if let Some(k) = digit_to_keysym(usage.0) {
        return Some(k);
    }
    Some(match usage {
        HidUsage::ENTER => 0xff0d,         // XK_Return
        HidUsage::ESCAPE => 0xff1b,        // XK_Escape
        HidUsage::BACKSPACE => 0xff08,     // XK_BackSpace
        HidUsage::TAB => 0xff09,           // XK_Tab
        HidUsage::SPACE => 0x0020,         // XK_space
        HidUsage::MINUS => 0x002d,         // XK_minus
        HidUsage::EQUAL => 0x003d,         // XK_equal
        HidUsage::LEFT_BRACKET => 0x005b,  // XK_bracketleft
        HidUsage::RIGHT_BRACKET => 0x005d, // XK_bracketright
        HidUsage::BACKSLASH => 0x005c,     // XK_backslash
        HidUsage::SEMICOLON => 0x003b,     // XK_semicolon
        HidUsage::APOSTROPHE => 0x0027,    // XK_apostrophe
        HidUsage::GRAVE => 0x0060,         // XK_grave
        HidUsage::COMMA => 0x002c,         // XK_comma
        HidUsage::PERIOD => 0x002e,        // XK_period
        HidUsage::SLASH => 0x002f,         // XK_slash
        HidUsage::CAPS_LOCK => 0xffe5,     // XK_Caps_Lock
        HidUsage::F1 => 0xffbe,            // XK_F1
        HidUsage::F2 => 0xffbf,
        HidUsage::F3 => 0xffc0,
        HidUsage::F4 => 0xffc1,
        HidUsage::F5 => 0xffc2,
        HidUsage::F6 => 0xffc3,
        HidUsage::F7 => 0xffc4,
        HidUsage::F8 => 0xffc5,
        HidUsage::F9 => 0xffc6,
        HidUsage::F10 => 0xffc7,
        HidUsage::F11 => 0xffc8,
        HidUsage::F12 => 0xffc9,
        HidUsage::F13 => 0xffca,
        HidUsage::F14 => 0xffcb,
        HidUsage::F15 => 0xffcc,
        HidUsage::F16 => 0xffcd,
        HidUsage::F17 => 0xffce,
        HidUsage::F18 => 0xffcf,
        HidUsage::F19 => 0xffd0,
        HidUsage::F20 => 0xffd1,
        HidUsage::F21 => 0xffd2,
        HidUsage::F22 => 0xffd3,
        HidUsage::F23 => 0xffd4,
        HidUsage::F24 => 0xffd5,
        HidUsage::PRINT_SCREEN => 0xff61, // XK_Print
        HidUsage::SCROLL_LOCK => 0xff14,  // XK_Scroll_Lock
        HidUsage::PAUSE => 0xff13,        // XK_Pause
        HidUsage::INSERT => 0xff63,       // XK_Insert
        HidUsage::HOME => 0xff50,         // XK_Home
        HidUsage::PAGE_UP => 0xff55,      // XK_Page_Up
        HidUsage::DELETE => 0xffff,       // XK_Delete
        HidUsage::END => 0xff57,          // XK_End
        HidUsage::PAGE_DOWN => 0xff56,    // XK_Page_Down
        HidUsage::RIGHT => 0xff53,        // XK_Right
        HidUsage::LEFT => 0xff51,         // XK_Left
        HidUsage::DOWN => 0xff54,         // XK_Down
        HidUsage::UP => 0xff52,           // XK_Up
        HidUsage::NUM_LOCK => 0xff7f,     // XK_Num_Lock
        HidUsage::KP_SLASH => 0xffaf,
        HidUsage::KP_ASTERISK => 0xffaa,
        HidUsage::KP_MINUS => 0xffad,
        HidUsage::KP_PLUS => 0xffab,
        HidUsage::KP_ENTER => 0xff8d,
        HidUsage::KP_1 => 0xffb1,
        HidUsage::KP_2 => 0xffb2,
        HidUsage::KP_3 => 0xffb3,
        HidUsage::KP_4 => 0xffb4,
        HidUsage::KP_5 => 0xffb5,
        HidUsage::KP_6 => 0xffb6,
        HidUsage::KP_7 => 0xffb7,
        HidUsage::KP_8 => 0xffb8,
        HidUsage::KP_9 => 0xffb9,
        HidUsage::KP_0 => 0xffb0,
        HidUsage::KP_PERIOD => 0xffae,
        HidUsage::APPLICATION => 0xff67, // XK_Menu
        HidUsage::LCTRL => 0xffe3,
        HidUsage::LSHIFT => 0xffe1,
        HidUsage::LALT => 0xffe9,
        HidUsage::LGUI => 0xffeb, // XK_Super_L
        HidUsage::RCTRL => 0xffe4,
        HidUsage::RSHIFT => 0xffe2,
        HidUsage::RALT => 0xffea,
        HidUsage::RGUI => 0xffec,
        _ => return None,
    })
}

/// Inverse of [`hid_to_keysym`] for the keysyms we know about.
///
/// Used by the capture path: native keycode → keysym (via xkb / X11
/// keymap) → HID. Returns `None` for unrecognised keysyms.
pub fn keysym_to_hid(keysym: Keysym) -> Option<HidUsage> {
    // Lowercase ASCII letters.
    if (0x0061..=0x007a).contains(&keysym) {
        return Some(HidUsage((keysym - 0x0061) as u16 + 0x04));
    }
    // Uppercase letters (some keymaps deliver these directly).
    if (0x0041..=0x005a).contains(&keysym) {
        return Some(HidUsage((keysym - 0x0041) as u16 + 0x04));
    }
    // Digits 0..9 -> HID order: '0' is 0x27, '1'..'9' is 0x1E..0x26.
    if keysym == 0x0030 {
        return Some(HidUsage::KEY_0);
    }
    if (0x0031..=0x0039).contains(&keysym) {
        return Some(HidUsage((keysym - 0x0031) as u16 + 0x1E));
    }
    Some(match keysym {
        0xff0d => HidUsage::ENTER,
        0xff1b => HidUsage::ESCAPE,
        0xff08 => HidUsage::BACKSPACE,
        0xff09 => HidUsage::TAB,
        0x0020 => HidUsage::SPACE,
        0x002d => HidUsage::MINUS,
        0x003d => HidUsage::EQUAL,
        0x005b => HidUsage::LEFT_BRACKET,
        0x005d => HidUsage::RIGHT_BRACKET,
        0x005c => HidUsage::BACKSLASH,
        0x003b => HidUsage::SEMICOLON,
        0x0027 => HidUsage::APOSTROPHE,
        0x0060 => HidUsage::GRAVE,
        0x002c => HidUsage::COMMA,
        0x002e => HidUsage::PERIOD,
        0x002f => HidUsage::SLASH,
        0xffe5 => HidUsage::CAPS_LOCK,
        0xffbe => HidUsage::F1,
        0xffbf => HidUsage::F2,
        0xffc0 => HidUsage::F3,
        0xffc1 => HidUsage::F4,
        0xffc2 => HidUsage::F5,
        0xffc3 => HidUsage::F6,
        0xffc4 => HidUsage::F7,
        0xffc5 => HidUsage::F8,
        0xffc6 => HidUsage::F9,
        0xffc7 => HidUsage::F10,
        0xffc8 => HidUsage::F11,
        0xffc9 => HidUsage::F12,
        0xff63 => HidUsage::INSERT,
        0xff50 => HidUsage::HOME,
        0xff55 => HidUsage::PAGE_UP,
        0xffff => HidUsage::DELETE,
        0xff57 => HidUsage::END,
        0xff56 => HidUsage::PAGE_DOWN,
        0xff53 => HidUsage::RIGHT,
        0xff51 => HidUsage::LEFT,
        0xff54 => HidUsage::DOWN,
        0xff52 => HidUsage::UP,
        0xff7f => HidUsage::NUM_LOCK,
        0xffe3 => HidUsage::LCTRL,
        0xffe1 => HidUsage::LSHIFT,
        0xffe9 => HidUsage::LALT,
        0xffeb => HidUsage::LGUI,
        0xffe4 => HidUsage::RCTRL,
        0xffe2 => HidUsage::RSHIFT,
        0xffea => HidUsage::RALT,
        0xffec => HidUsage::RGUI,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabetic_round_trip() {
        for usage in 0x04..=0x1D {
            let k = hid_to_keysym(HidUsage(usage)).unwrap();
            let back = keysym_to_hid(k).unwrap();
            assert_eq!(back, HidUsage(usage));
        }
    }

    #[test]
    fn digits_round_trip() {
        for usage in 0x1E..=0x27 {
            let k = hid_to_keysym(HidUsage(usage)).unwrap();
            let back = keysym_to_hid(k).unwrap();
            assert_eq!(back, HidUsage(usage));
        }
    }

    #[test]
    fn modifier_round_trip() {
        for &m in &[
            HidUsage::LCTRL,
            HidUsage::LSHIFT,
            HidUsage::LALT,
            HidUsage::LGUI,
            HidUsage::RCTRL,
            HidUsage::RSHIFT,
            HidUsage::RALT,
            HidUsage::RGUI,
        ] {
            let k = hid_to_keysym(m).unwrap();
            assert_eq!(keysym_to_hid(k).unwrap(), m);
        }
    }
}
