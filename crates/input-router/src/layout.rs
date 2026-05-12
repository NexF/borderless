//! Logical screen topology.

use borderless_core::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One side of a screen rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Edge {
    /// Left.
    Left,
    /// Right.
    Right,
    /// Top.
    Top,
    /// Bottom.
    Bottom,
}

impl Edge {
    /// Edge of the *destination* screen the cursor enters from.
    pub fn opposite(self) -> Edge {
        match self {
            Edge::Left => Edge::Right,
            Edge::Right => Edge::Left,
            Edge::Top => Edge::Bottom,
            Edge::Bottom => Edge::Top,
        }
    }
}

/// One screen in the virtual layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    /// Owning node.
    pub node: NodeId,
    /// Logical width in pixels (informational; we cross by edge, not by coord).
    pub width: u32,
    /// Logical height in pixels.
    pub height: u32,
}

/// The full virtual layout. We index nodes by [`NodeId`] and store edge
/// adjacencies as a sparse map.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    screens: HashMap<NodeId, Screen>,
    /// `(node, edge)` -> the node you reach by walking off `edge`.
    adjacency: HashMap<(NodeId, Edge), NodeId>,
}

impl Layout {
    /// Empty layout.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register or replace a screen.
    pub fn set_screen(&mut self, screen: Screen) {
        self.screens.insert(screen.node, screen);
    }

    /// Connect `from`'s `edge` to `to` (and the reciprocal opposite
    /// edge for symmetry).
    pub fn connect(&mut self, from: NodeId, edge: Edge, to: NodeId) {
        self.adjacency.insert((from, edge), to);
        self.adjacency.insert((to, edge.opposite()), from);
    }

    /// Lookup neighbor on `edge`.
    pub fn neighbor(&self, node: NodeId, edge: Edge) -> Option<NodeId> {
        self.adjacency.get(&(node, edge)).copied()
    }

    /// True if we have any registered screen.
    pub fn is_empty(&self) -> bool {
        self.screens.is_empty()
    }

    /// Get a screen by node.
    pub fn screen(&self, node: NodeId) -> Option<&Screen> {
        self.screens.get(&node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(byte: u8) -> NodeId {
        NodeId([byte; 16])
    }

    #[test]
    fn edge_opposite() {
        assert_eq!(Edge::Left.opposite(), Edge::Right);
        assert_eq!(Edge::Top.opposite(), Edge::Bottom);
    }

    #[test]
    fn connect_is_symmetric() {
        let a = n(1);
        let b = n(2);
        let mut l = Layout::new();
        l.set_screen(Screen { node: a, width: 1920, height: 1080 });
        l.set_screen(Screen { node: b, width: 1920, height: 1080 });
        l.connect(a, Edge::Right, b);
        assert_eq!(l.neighbor(a, Edge::Right), Some(b));
        assert_eq!(l.neighbor(b, Edge::Left), Some(a));
        assert_eq!(l.neighbor(a, Edge::Top), None);
    }
}
