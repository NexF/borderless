//! Input event types shared between active and passive nodes.

use crate::hid::{HidUsage, ModifierMask};
use crate::node::NodeId;
use serde::{Deserialize, Serialize};

/// Mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Button {
    /// Left button (primary).
    Left,
    /// Right button (secondary).
    Right,
    /// Middle button.
    Middle,
    /// Back button (X1).
    Back,
    /// Forward button (X2).
    Forward,
    /// Vendor-defined / unknown button id.
    Other(u8),
}

/// A single input event flowing from the Active node to a Passive node.
///
/// Mouse moves are intentionally **delta-based** so that nodes with
/// different DPI / resolution don't need to negotiate a coordinate
/// space: each receiver applies the delta in its own pixel units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputEvent {
    /// Pointer moved by `(dx, dy)` device pixels.
    /// `ts` is a millisecond timestamp from the source's monotonic
    /// clock, used for jitter measurements.
    MouseMove {
        /// X delta.
        dx: i32,
        /// Y delta.
        dy: i32,
        /// Source-side monotonic timestamp in milliseconds.
        ts: u64,
    },
    /// Pointer warped to absolute coordinates inside the named screen.
    MouseAbs {
        /// X coordinate.
        x: i32,
        /// Y coordinate.
        y: i32,
        /// Logical screen index in the layout.
        screen_id: u8,
    },
    /// Mouse button state change.
    MouseButton {
        /// Button.
        btn: Button,
        /// True for press, false for release.
        pressed: bool,
    },
    /// Scroll wheel; positive y = scroll down, positive x = scroll right.
    Scroll {
        /// X delta.
        dx: i32,
        /// Y delta.
        dy: i32,
    },
    /// Keyboard event using a USB HID usage code.
    Key {
        /// HID usage.
        code: HidUsage,
        /// True for press, false for release.
        pressed: bool,
        /// Snapshot of currently held modifiers.
        modifiers: ModifierMask,
    },
    /// Cursor entered this peer's logical screen.
    ///
    /// MUST be sent before any other input event so the receiving side
    /// can synthesize the modifier state held on the source. Without
    /// this, a `Shift` held while crossing a screen boundary would
    /// leak into the next keystroke as `shift+letter`.
    Enter {
        /// Source node.
        from: NodeId,
        /// Modifier state at the moment of entry.
        modifiers: ModifierMask,
    },
    /// Cursor left this peer's logical screen.
    Leave {
        /// Destination node.
        to: NodeId,
    },
}
