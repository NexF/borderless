//! Edge-crossing router.
//!
//! The router holds the Active node's current cursor position and
//! decides, on each pointer delta, whether the cursor should be
//! handed off to a neighbor. When that happens it emits a
//! `(Leave, Enter)` pair targeted at the source and destination.

use crate::layout::{Edge, Layout};
use crate::modifier::ModifierState;
use borderless_core::{InputEvent, NodeId};

/// One routing decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Routed {
    /// Stay on the local screen; deliver the event locally as-is.
    Local(InputEvent),
    /// Forward this event to the named remote node.
    Remote {
        /// Receiving node.
        target: NodeId,
        /// Event to forward.
        event: InputEvent,
    },
}

/// Edge-crossing router state. One per Active node.
#[derive(Debug)]
pub struct Router {
    /// Self id.
    pub self_id: NodeId,
    /// Currently-focused node (the "logical" cursor location). When
    /// equal to `self_id` we own the cursor; otherwise we forward.
    pub active: NodeId,
    /// Modifier-key state, shipped with Enter events.
    pub modifiers: ModifierState,
    /// Cursor position relative to the *active* node's screen.
    pub x: i32,
    /// Cursor position relative to the *active* node's screen.
    pub y: i32,
    /// Width of the local screen.
    pub width: i32,
    /// Height of the local screen.
    pub height: i32,
}

impl Router {
    /// Construct.
    pub fn new(self_id: NodeId, width: u32, height: u32) -> Self {
        Self {
            self_id,
            active: self_id,
            modifiers: ModifierState::new(),
            x: width as i32 / 2,
            y: height as i32 / 2,
            width: width as i32,
            height: height as i32,
        }
    }

    /// Handle a local mouse delta. Returns the `(Leave, Enter)` pair if
    /// the cursor crossed a boundary in `layout`, otherwise an empty
    /// vec if no message needs to be sent (the local cursor moved but
    /// stayed on the same screen).
    pub fn on_mouse_move(&mut self, dx: i32, dy: i32, ts: u64, layout: &Layout) -> Vec<Routed> {
        let mut out = Vec::new();
        // If currently active is remote, forward the delta as-is.
        if self.active != self.self_id {
            out.push(Routed::Remote {
                target: self.active,
                event: InputEvent::MouseMove { dx, dy, ts },
            });
            return out;
        }

        self.x += dx;
        self.y += dy;

        let crossed_edge = if self.x < 0 {
            Some(Edge::Left)
        } else if self.x >= self.width {
            Some(Edge::Right)
        } else if self.y < 0 {
            Some(Edge::Top)
        } else if self.y >= self.height {
            Some(Edge::Bottom)
        } else {
            None
        };

        match crossed_edge.and_then(|e| layout.neighbor(self.self_id, e).map(|n| (e, n))) {
            None => {
                // Clamp inside, deliver locally.
                self.x = self.x.clamp(0, self.width.saturating_sub(1));
                self.y = self.y.clamp(0, self.height.saturating_sub(1));
                out.push(Routed::Local(InputEvent::MouseMove { dx, dy, ts }));
            }
            Some((_edge, next_node)) => {
                let modifiers = self.modifiers.mask();
                out.push(Routed::Remote {
                    target: self.self_id,
                    event: InputEvent::Leave { to: next_node },
                });
                out.push(Routed::Remote {
                    target: next_node,
                    event: InputEvent::Enter {
                        from: self.self_id,
                        modifiers,
                    },
                });
                self.active = next_node;
                // We don't track the remote screen size; reset our
                // local bookkeeping so the next return-cross works.
                self.x = self.width / 2;
                self.y = self.height / 2;
            }
        }
        out
    }

    /// Handle a key event; updates modifier state as a side effect.
    pub fn on_key(&mut self, event: InputEvent) -> Routed {
        if let InputEvent::Key { code, pressed, .. } = event {
            self.modifiers.update(code, pressed);
        }
        if self.active == self.self_id {
            Routed::Local(event)
        } else {
            Routed::Remote {
                target: self.active,
                event,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Layout, Screen};
    use borderless_core::ModifierMask;

    fn n(byte: u8) -> NodeId {
        NodeId([byte; 16])
    }

    fn two_node_horizontal() -> (Layout, NodeId, NodeId) {
        let a = n(1);
        let b = n(2);
        let mut l = Layout::new();
        l.set_screen(Screen { node: a, width: 1920, height: 1080 });
        l.set_screen(Screen { node: b, width: 1920, height: 1080 });
        l.connect(a, Edge::Right, b);
        (l, a, b)
    }

    #[test]
    fn move_inside_stays_local() {
        let (layout, a, _b) = two_node_horizontal();
        let mut r = Router::new(a, 1920, 1080);
        let out = r.on_mouse_move(10, 0, 0, &layout);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], Routed::Local(_)));
    }

    #[test]
    fn cross_right_edge_emits_leave_and_enter() {
        let (layout, a, b) = two_node_horizontal();
        let mut r = Router::new(a, 1920, 1080);
        // Force cursor near the right edge.
        r.x = 1919;
        let out = r.on_mouse_move(50, 0, 0, &layout);
        assert_eq!(out.len(), 2);
        match &out[0] {
            Routed::Remote { target, event: InputEvent::Leave { to } } => {
                assert_eq!(*target, a);
                assert_eq!(*to, b);
            }
            _ => panic!("expected Leave to {b:?}"),
        }
        match &out[1] {
            Routed::Remote {
                target,
                event: InputEvent::Enter { from, modifiers: _ },
            } => {
                assert_eq!(*target, b);
                assert_eq!(*from, a);
            }
            _ => panic!("expected Enter from {a:?}"),
        }
        assert_eq!(r.active, b);
    }

    #[test]
    fn modifiers_carry_through_enter() {
        let (layout, a, _b) = two_node_horizontal();
        let mut r = Router::new(a, 1920, 1080);
        r.modifiers.update(borderless_core::HidUsage::LSHIFT, true);
        r.x = 1919;
        let out = r.on_mouse_move(50, 0, 0, &layout);
        if let Routed::Remote {
            event: InputEvent::Enter { modifiers, .. },
            ..
        } = &out[1]
        {
            assert_eq!(*modifiers, ModifierMask::LSHIFT);
        } else {
            panic!("expected Enter");
        }
    }

    #[test]
    fn key_event_forwarded_when_active_is_remote() {
        let (_layout, a, b) = two_node_horizontal();
        let mut r = Router::new(a, 1920, 1080);
        r.active = b;
        let key = InputEvent::Key {
            code: borderless_core::HidUsage::KEY_A,
            pressed: true,
            modifiers: ModifierMask::empty(),
        };
        match r.on_key(key) {
            Routed::Remote { target, event } => {
                assert_eq!(target, b);
                assert_eq!(event, key);
            }
            other => panic!("expected forwarding, got {other:?}"),
        }
    }

    #[test]
    fn key_event_local_when_active_is_self() {
        let (_layout, a, _b) = two_node_horizontal();
        let mut r = Router::new(a, 1920, 1080);
        let key = InputEvent::Key {
            code: borderless_core::HidUsage::ENTER,
            pressed: false,
            modifiers: ModifierMask::empty(),
        };
        assert!(matches!(r.on_key(key), Routed::Local(_)));
    }

    #[test]
    fn key_press_release_updates_modifier_state_through_router() {
        let (_layout, a, _b) = two_node_horizontal();
        let mut r = Router::new(a, 1920, 1080);
        r.on_key(InputEvent::Key {
            code: borderless_core::HidUsage::LCTRL,
            pressed: true,
            modifiers: ModifierMask::empty(),
        });
        assert_eq!(r.modifiers.mask(), ModifierMask::LCTRL);
        r.on_key(InputEvent::Key {
            code: borderless_core::HidUsage::LCTRL,
            pressed: false,
            modifiers: ModifierMask::LCTRL,
        });
        assert_eq!(r.modifiers.mask(), ModifierMask::empty());
    }

    #[test]
    fn cross_into_unmapped_edge_clamps_locally() {
        // A's right is B; left is unmapped. Walking off the left edge
        // must clamp inside, not panic, not forward.
        let (layout, a, _b) = two_node_horizontal();
        let mut r = Router::new(a, 1920, 1080);
        r.x = 0;
        let out = r.on_mouse_move(-50, 0, 0, &layout);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], Routed::Local(_)));
        assert_eq!(r.x, 0, "cursor should clamp at the left edge");
        assert_eq!(r.active, a, "still local");
    }

    #[test]
    fn after_crossing_moves_forward_to_remote() {
        let (layout, a, b) = two_node_horizontal();
        let mut r = Router::new(a, 1920, 1080);
        r.x = 1919;
        let _ = r.on_mouse_move(50, 0, 0, &layout);
        let out = r.on_mouse_move(5, 5, 1, &layout);
        assert_eq!(out.len(), 1);
        match &out[0] {
            Routed::Remote {
                target,
                event: InputEvent::MouseMove { dx, dy, .. },
            } => {
                assert_eq!(*target, b);
                assert_eq!(*dx, 5);
                assert_eq!(*dy, 5);
            }
            _ => panic!("expected forwarded MouseMove"),
        }
    }
}
