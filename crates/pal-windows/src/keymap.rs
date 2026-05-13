//! Static HID Usage ↔ Windows Virtual Key + Scan code lookup.
//!
//! Windows hooks deliver both VK codes and scan codes; for `SendInput`
//! we prefer to send scan codes (the `KEYEVENTF_SCANCODE` flag) since
//! they survive layout changes and remote-desktop quirks better.

use borderless_core::HidUsage;

/// (VK, scan_code, is_extended).
pub type WinKey = (u16, u16, bool);

/// HID -> (VK, scan code, extended bit).
///
/// Returns `None` for unknown HID codes so the caller can drop them.
pub fn hid_to_win(hid: HidUsage) -> Option<WinKey> {
    let n = hid.0;
    // Letters a..z -> VK_A..VK_Z (0x41..0x5A), scan codes from
    // standard US 101-key layout.
    if (0x04..=0x1D).contains(&n) {
        let idx = n - 0x04;
        let vk = 0x41 + idx;
        let scan = LETTER_SCANCODES[idx as usize];
        return Some((vk, scan, false));
    }
    // Digits 1..0 -> VK_1..VK_0.
    if (0x1E..=0x27).contains(&n) {
        let idx = n - 0x1E;
        let vk = if n == 0x27 { 0x30 } else { 0x31 + idx };
        let scan = DIGIT_SCANCODES[idx as usize];
        return Some((vk, scan, false));
    }
    Some(match hid {
        HidUsage::ENTER => (0x0D, 0x1C, false),
        HidUsage::ESCAPE => (0x1B, 0x01, false),
        HidUsage::BACKSPACE => (0x08, 0x0E, false),
        HidUsage::TAB => (0x09, 0x0F, false),
        HidUsage::SPACE => (0x20, 0x39, false),
        HidUsage::MINUS => (0xBD, 0x0C, false),
        HidUsage::EQUAL => (0xBB, 0x0D, false),
        HidUsage::LEFT_BRACKET => (0xDB, 0x1A, false),
        HidUsage::RIGHT_BRACKET => (0xDD, 0x1B, false),
        HidUsage::BACKSLASH => (0xDC, 0x2B, false),
        HidUsage::SEMICOLON => (0xBA, 0x27, false),
        HidUsage::APOSTROPHE => (0xDE, 0x28, false),
        HidUsage::GRAVE => (0xC0, 0x29, false),
        HidUsage::COMMA => (0xBC, 0x33, false),
        HidUsage::PERIOD => (0xBE, 0x34, false),
        HidUsage::SLASH => (0xBF, 0x35, false),
        HidUsage::CAPS_LOCK => (0x14, 0x3A, false),
        HidUsage::F1 => (0x70, 0x3B, false),
        HidUsage::F2 => (0x71, 0x3C, false),
        HidUsage::F3 => (0x72, 0x3D, false),
        HidUsage::F4 => (0x73, 0x3E, false),
        HidUsage::F5 => (0x74, 0x3F, false),
        HidUsage::F6 => (0x75, 0x40, false),
        HidUsage::F7 => (0x76, 0x41, false),
        HidUsage::F8 => (0x77, 0x42, false),
        HidUsage::F9 => (0x78, 0x43, false),
        HidUsage::F10 => (0x79, 0x44, false),
        HidUsage::F11 => (0x7A, 0x57, false),
        HidUsage::F12 => (0x7B, 0x58, false),
        HidUsage::PRINT_SCREEN => (0x2C, 0x37, true),
        HidUsage::SCROLL_LOCK => (0x91, 0x46, false),
        HidUsage::PAUSE => (0x13, 0x45, false),
        HidUsage::INSERT => (0x2D, 0x52, true),
        HidUsage::HOME => (0x24, 0x47, true),
        HidUsage::PAGE_UP => (0x21, 0x49, true),
        HidUsage::DELETE => (0x2E, 0x53, true),
        HidUsage::END => (0x23, 0x4F, true),
        HidUsage::PAGE_DOWN => (0x22, 0x51, true),
        HidUsage::RIGHT => (0x27, 0x4D, true),
        HidUsage::LEFT => (0x25, 0x4B, true),
        HidUsage::DOWN => (0x28, 0x50, true),
        HidUsage::UP => (0x26, 0x48, true),
        HidUsage::NUM_LOCK => (0x90, 0x45, true),
        HidUsage::KP_SLASH => (0x6F, 0x35, true),
        HidUsage::KP_ASTERISK => (0x6A, 0x37, false),
        HidUsage::KP_MINUS => (0x6D, 0x4A, false),
        HidUsage::KP_PLUS => (0x6B, 0x4E, false),
        HidUsage::KP_ENTER => (0x0D, 0x1C, true),
        HidUsage::KP_1 => (0x61, 0x4F, false),
        HidUsage::KP_2 => (0x62, 0x50, false),
        HidUsage::KP_3 => (0x63, 0x51, false),
        HidUsage::KP_4 => (0x64, 0x4B, false),
        HidUsage::KP_5 => (0x65, 0x4C, false),
        HidUsage::KP_6 => (0x66, 0x4D, false),
        HidUsage::KP_7 => (0x67, 0x47, false),
        HidUsage::KP_8 => (0x68, 0x48, false),
        HidUsage::KP_9 => (0x69, 0x49, false),
        HidUsage::KP_0 => (0x60, 0x52, false),
        HidUsage::KP_PERIOD => (0x6E, 0x53, false),
        HidUsage::APPLICATION => (0x5D, 0x5D, true),
        HidUsage::LCTRL => (0xA2, 0x1D, false),
        HidUsage::LSHIFT => (0xA0, 0x2A, false),
        HidUsage::LALT => (0xA4, 0x38, false),
        HidUsage::LGUI => (0x5B, 0x5B, true),
        HidUsage::RCTRL => (0xA3, 0x1D, true),
        HidUsage::RSHIFT => (0xA1, 0x36, false),
        HidUsage::RALT => (0xA5, 0x38, true),
        HidUsage::RGUI => (0x5C, 0x5C, true),
        _ => return None,
    })
}

/// Reverse map: VK code → HID. The scan-code path is more accurate
/// but VK-only is fine for the v0.2 hook surface where the OS gives
/// us a scan code anyway.
pub fn vk_to_hid(vk: u16) -> Option<HidUsage> {
    if (0x41..=0x5A).contains(&vk) {
        return Some(HidUsage((vk - 0x41) as u16 + 0x04));
    }
    if vk == 0x30 {
        return Some(HidUsage::KEY_0);
    }
    if (0x31..=0x39).contains(&vk) {
        return Some(HidUsage((vk - 0x31) as u16 + 0x1E));
    }
    Some(match vk {
        0x0D => HidUsage::ENTER,
        0x1B => HidUsage::ESCAPE,
        0x08 => HidUsage::BACKSPACE,
        0x09 => HidUsage::TAB,
        0x20 => HidUsage::SPACE,
        0xBD => HidUsage::MINUS,
        0xBB => HidUsage::EQUAL,
        0xDB => HidUsage::LEFT_BRACKET,
        0xDD => HidUsage::RIGHT_BRACKET,
        0xDC => HidUsage::BACKSLASH,
        0xBA => HidUsage::SEMICOLON,
        0xDE => HidUsage::APOSTROPHE,
        0xC0 => HidUsage::GRAVE,
        0xBC => HidUsage::COMMA,
        0xBE => HidUsage::PERIOD,
        0xBF => HidUsage::SLASH,
        0x14 => HidUsage::CAPS_LOCK,
        0x70 => HidUsage::F1,
        0x71 => HidUsage::F2,
        0x72 => HidUsage::F3,
        0x73 => HidUsage::F4,
        0x74 => HidUsage::F5,
        0x75 => HidUsage::F6,
        0x76 => HidUsage::F7,
        0x77 => HidUsage::F8,
        0x78 => HidUsage::F9,
        0x79 => HidUsage::F10,
        0x7A => HidUsage::F11,
        0x7B => HidUsage::F12,
        0x2C => HidUsage::PRINT_SCREEN,
        0x91 => HidUsage::SCROLL_LOCK,
        0x13 => HidUsage::PAUSE,
        0x2D => HidUsage::INSERT,
        0x24 => HidUsage::HOME,
        0x21 => HidUsage::PAGE_UP,
        0x2E => HidUsage::DELETE,
        0x23 => HidUsage::END,
        0x22 => HidUsage::PAGE_DOWN,
        0x27 => HidUsage::RIGHT,
        0x25 => HidUsage::LEFT,
        0x28 => HidUsage::DOWN,
        0x26 => HidUsage::UP,
        0x90 => HidUsage::NUM_LOCK,
        0xA2 => HidUsage::LCTRL,
        0xA0 => HidUsage::LSHIFT,
        0xA4 => HidUsage::LALT,
        0x5B => HidUsage::LGUI,
        0xA3 => HidUsage::RCTRL,
        0xA1 => HidUsage::RSHIFT,
        0xA5 => HidUsage::RALT,
        0x5C => HidUsage::RGUI,
        _ => return None,
    })
}

const LETTER_SCANCODES: [u16; 26] = [
    0x1E, 0x30, 0x2E, 0x20, 0x12, 0x21, 0x22, 0x23, // a..h
    0x17, 0x24, 0x25, 0x26, 0x32, 0x31, 0x18, 0x19, // i..p
    0x10, 0x13, 0x1F, 0x14, 0x16, 0x2F, 0x11, 0x2D, // q..x
    0x15, 0x2C, // y, z
];

const DIGIT_SCANCODES: [u16; 10] = [
    0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, // 1..9
    0x0B, // 0
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_round_trip() {
        for usage in 0x04..=0x1D {
            let (vk, _, _) = hid_to_win(HidUsage(usage)).unwrap();
            let back = vk_to_hid(vk).unwrap();
            assert_eq!(back, HidUsage(usage));
        }
    }

    #[test]
    fn digit_round_trip() {
        for usage in 0x1E..=0x27 {
            let (vk, _, _) = hid_to_win(HidUsage(usage)).unwrap();
            let back = vk_to_hid(vk).unwrap();
            assert_eq!(back, HidUsage(usage));
        }
    }

    #[test]
    fn extended_keys_set_bit() {
        for k in [
            HidUsage::INSERT,
            HidUsage::HOME,
            HidUsage::PAGE_UP,
            HidUsage::DELETE,
            HidUsage::END,
            HidUsage::PAGE_DOWN,
            HidUsage::RIGHT,
            HidUsage::LEFT,
            HidUsage::DOWN,
            HidUsage::UP,
            HidUsage::RCTRL,
            HidUsage::RALT,
            HidUsage::LGUI,
            HidUsage::RGUI,
            HidUsage::KP_ENTER,
        ] {
            let (_, _, ext) = hid_to_win(k).unwrap();
            assert!(ext, "{k:?} should be extended");
        }
    }
}
