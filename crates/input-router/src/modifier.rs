//! Modifier-key state machine.
//!
//! The Active node tracks which modifiers (`Shift`, `Ctrl`, `Alt`,
//! `Cmd/Win`) are currently *held*. Whenever the cursor crosses a
//! boundary, this snapshot is shipped along with the
//! [`InputEvent::Enter`](borderless_core::InputEvent::Enter) frame so
//! the destination starts from the right baseline.

use borderless_core::{HidUsage, ModifierMask};

/// Tracks which modifier keys are currently down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModifierState {
    mask: ModifierMask,
}

impl ModifierState {
    /// Empty (nothing held).
    pub fn new() -> Self {
        Self::default()
    }

    /// Current bitmask.
    pub fn mask(&self) -> ModifierMask {
        self.mask
    }

    /// Apply a key press/release. Returns `true` if `code` is a
    /// modifier and the mask changed.
    pub fn update(&mut self, code: HidUsage, pressed: bool) -> bool {
        let bit = match code {
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
        let Some(bit) = bit else {
            return false;
        };
        let before = self.mask;
        if pressed {
            self.mask |= bit;
        } else {
            self.mask -= bit;
        }
        before != self.mask
    }

    /// Reset all modifiers (call on `Leave` so the source machine
    /// doesn't leak state if the user releases keys after crossing).
    pub fn clear(&mut self) {
        self.mask = ModifierMask::empty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_press_release() {
        let mut s = ModifierState::new();
        assert!(s.update(HidUsage::LSHIFT, true));
        assert_eq!(s.mask(), ModifierMask::LSHIFT);
        assert!(!s.update(HidUsage::KEY_A, true), "a is not a modifier");
        assert!(s.update(HidUsage::LSHIFT, false));
        assert_eq!(s.mask(), ModifierMask::empty());
    }

    #[test]
    fn shift_plus_ctrl_combine() {
        let mut s = ModifierState::new();
        s.update(HidUsage::LSHIFT, true);
        s.update(HidUsage::LCTRL, true);
        assert_eq!(s.mask(), ModifierMask::LSHIFT | ModifierMask::LCTRL);
        s.update(HidUsage::LSHIFT, false);
        assert_eq!(s.mask(), ModifierMask::LCTRL);
    }
}
