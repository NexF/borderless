//! Static HID Usage ↔ macOS virtual keycode lookup.
//!
//! Reference: `/System/Library/Frameworks/Carbon.framework/Versions/A/Headers/Events.h`
//! (the `kVK_*` constants).

use borderless_core::HidUsage;

/// Translate a HID usage to a macOS CGKeyCode (`u16`).
pub fn hid_to_mac(hid: HidUsage) -> Option<u16> {
    Some(match hid {
        HidUsage::KEY_A => 0x00,
        HidUsage::KEY_B => 0x0B,
        HidUsage::KEY_C => 0x08,
        HidUsage::KEY_D => 0x02,
        HidUsage::KEY_E => 0x0E,
        HidUsage::KEY_F => 0x03,
        HidUsage::KEY_G => 0x05,
        HidUsage::KEY_H => 0x04,
        HidUsage::KEY_I => 0x22,
        HidUsage::KEY_J => 0x26,
        HidUsage::KEY_K => 0x28,
        HidUsage::KEY_L => 0x25,
        HidUsage::KEY_M => 0x2E,
        HidUsage::KEY_N => 0x2D,
        HidUsage::KEY_O => 0x1F,
        HidUsage::KEY_P => 0x23,
        HidUsage::KEY_Q => 0x0C,
        HidUsage::KEY_R => 0x0F,
        HidUsage::KEY_S => 0x01,
        HidUsage::KEY_T => 0x11,
        HidUsage::KEY_U => 0x20,
        HidUsage::KEY_V => 0x09,
        HidUsage::KEY_W => 0x0D,
        HidUsage::KEY_X => 0x07,
        HidUsage::KEY_Y => 0x10,
        HidUsage::KEY_Z => 0x06,
        HidUsage::KEY_1 => 0x12,
        HidUsage::KEY_2 => 0x13,
        HidUsage::KEY_3 => 0x14,
        HidUsage::KEY_4 => 0x15,
        HidUsage::KEY_5 => 0x17,
        HidUsage::KEY_6 => 0x16,
        HidUsage::KEY_7 => 0x1A,
        HidUsage::KEY_8 => 0x1C,
        HidUsage::KEY_9 => 0x19,
        HidUsage::KEY_0 => 0x1D,
        HidUsage::ENTER => 0x24,
        HidUsage::ESCAPE => 0x35,
        HidUsage::BACKSPACE => 0x33,
        HidUsage::TAB => 0x30,
        HidUsage::SPACE => 0x31,
        HidUsage::MINUS => 0x1B,
        HidUsage::EQUAL => 0x18,
        HidUsage::LEFT_BRACKET => 0x21,
        HidUsage::RIGHT_BRACKET => 0x1E,
        HidUsage::BACKSLASH => 0x2A,
        HidUsage::SEMICOLON => 0x29,
        HidUsage::APOSTROPHE => 0x27,
        HidUsage::GRAVE => 0x32,
        HidUsage::COMMA => 0x2B,
        HidUsage::PERIOD => 0x2F,
        HidUsage::SLASH => 0x2C,
        HidUsage::CAPS_LOCK => 0x39,
        HidUsage::F1 => 0x7A,
        HidUsage::F2 => 0x78,
        HidUsage::F3 => 0x63,
        HidUsage::F4 => 0x76,
        HidUsage::F5 => 0x60,
        HidUsage::F6 => 0x61,
        HidUsage::F7 => 0x62,
        HidUsage::F8 => 0x64,
        HidUsage::F9 => 0x65,
        HidUsage::F10 => 0x6D,
        HidUsage::F11 => 0x67,
        HidUsage::F12 => 0x6F,
        HidUsage::INSERT => 0x72, // Help on Mac
        HidUsage::HOME => 0x73,
        HidUsage::PAGE_UP => 0x74,
        HidUsage::DELETE => 0x75,
        HidUsage::END => 0x77,
        HidUsage::PAGE_DOWN => 0x79,
        HidUsage::RIGHT => 0x7C,
        HidUsage::LEFT => 0x7B,
        HidUsage::DOWN => 0x7D,
        HidUsage::UP => 0x7E,
        HidUsage::LCTRL => 0x3B,
        HidUsage::LSHIFT => 0x38,
        HidUsage::LALT => 0x3A, // Option
        HidUsage::LGUI => 0x37, // Command
        HidUsage::RCTRL => 0x3E,
        HidUsage::RSHIFT => 0x3C,
        HidUsage::RALT => 0x3D,
        HidUsage::RGUI => 0x36,
        _ => return None,
    })
}

/// Reverse map: macOS CGKeyCode → HID usage.
pub fn mac_to_hid(kc: u16) -> Option<HidUsage> {
    Some(match kc {
        0x00 => HidUsage::KEY_A,
        0x0B => HidUsage::KEY_B,
        0x08 => HidUsage::KEY_C,
        0x02 => HidUsage::KEY_D,
        0x0E => HidUsage::KEY_E,
        0x03 => HidUsage::KEY_F,
        0x05 => HidUsage::KEY_G,
        0x04 => HidUsage::KEY_H,
        0x22 => HidUsage::KEY_I,
        0x26 => HidUsage::KEY_J,
        0x28 => HidUsage::KEY_K,
        0x25 => HidUsage::KEY_L,
        0x2E => HidUsage::KEY_M,
        0x2D => HidUsage::KEY_N,
        0x1F => HidUsage::KEY_O,
        0x23 => HidUsage::KEY_P,
        0x0C => HidUsage::KEY_Q,
        0x0F => HidUsage::KEY_R,
        0x01 => HidUsage::KEY_S,
        0x11 => HidUsage::KEY_T,
        0x20 => HidUsage::KEY_U,
        0x09 => HidUsage::KEY_V,
        0x0D => HidUsage::KEY_W,
        0x07 => HidUsage::KEY_X,
        0x10 => HidUsage::KEY_Y,
        0x06 => HidUsage::KEY_Z,
        0x12 => HidUsage::KEY_1,
        0x13 => HidUsage::KEY_2,
        0x14 => HidUsage::KEY_3,
        0x15 => HidUsage::KEY_4,
        0x17 => HidUsage::KEY_5,
        0x16 => HidUsage::KEY_6,
        0x1A => HidUsage::KEY_7,
        0x1C => HidUsage::KEY_8,
        0x19 => HidUsage::KEY_9,
        0x1D => HidUsage::KEY_0,
        0x24 => HidUsage::ENTER,
        0x35 => HidUsage::ESCAPE,
        0x33 => HidUsage::BACKSPACE,
        0x30 => HidUsage::TAB,
        0x31 => HidUsage::SPACE,
        0x1B => HidUsage::MINUS,
        0x18 => HidUsage::EQUAL,
        0x21 => HidUsage::LEFT_BRACKET,
        0x1E => HidUsage::RIGHT_BRACKET,
        0x2A => HidUsage::BACKSLASH,
        0x29 => HidUsage::SEMICOLON,
        0x27 => HidUsage::APOSTROPHE,
        0x32 => HidUsage::GRAVE,
        0x2B => HidUsage::COMMA,
        0x2F => HidUsage::PERIOD,
        0x2C => HidUsage::SLASH,
        0x39 => HidUsage::CAPS_LOCK,
        0x7A => HidUsage::F1,
        0x78 => HidUsage::F2,
        0x63 => HidUsage::F3,
        0x76 => HidUsage::F4,
        0x60 => HidUsage::F5,
        0x61 => HidUsage::F6,
        0x62 => HidUsage::F7,
        0x64 => HidUsage::F8,
        0x65 => HidUsage::F9,
        0x6D => HidUsage::F10,
        0x67 => HidUsage::F11,
        0x6F => HidUsage::F12,
        0x72 => HidUsage::INSERT,
        0x73 => HidUsage::HOME,
        0x74 => HidUsage::PAGE_UP,
        0x75 => HidUsage::DELETE,
        0x77 => HidUsage::END,
        0x79 => HidUsage::PAGE_DOWN,
        0x7C => HidUsage::RIGHT,
        0x7B => HidUsage::LEFT,
        0x7D => HidUsage::DOWN,
        0x7E => HidUsage::UP,
        0x3B => HidUsage::LCTRL,
        0x38 => HidUsage::LSHIFT,
        0x3A => HidUsage::LALT,
        0x37 => HidUsage::LGUI,
        0x3E => HidUsage::RCTRL,
        0x3C => HidUsage::RSHIFT,
        0x3D => HidUsage::RALT,
        0x36 => HidUsage::RGUI,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_round_trip() {
        for usage in 0x04..=0x1D {
            let kc = hid_to_mac(HidUsage(usage)).unwrap();
            let back = mac_to_hid(kc).unwrap();
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
            let kc = hid_to_mac(m).unwrap();
            assert_eq!(mac_to_hid(kc).unwrap(), m);
        }
    }
}
