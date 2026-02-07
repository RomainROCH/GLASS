//! Retained scene graph with dirty-flag system.
//!
//! Scene nodes are created once and only re-uploaded to the GPU when they
//! change. In steady-state (no modifications), the render path produces
//! zero heap allocations and zero GPU buffer uploads.
//!
//! # Supported Node Types
//! - `Rect` — solid-color rectangle
//! - `Text` — text rendered via glyphon
//! - `Group` — ordered child nodes (future extension)

use std::fmt;

/// Unique identifier for a scene node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", self.0)
    }
}

/// RGBA colour (premultiplied alpha) for scene elements.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Pre-multiply RGB by alpha.
    pub fn premultiply(self) -> Self {
        Self {
            r: self.r * self.a,
            g: self.g * self.a,
            b: self.b * self.a,
            a: self.a,
        }
    }

    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0, 1.0);
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0, 1.0);
}

/// Properties for a rectangle node.
#[derive(Debug, Clone, PartialEq)]
pub struct RectProps {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: Color,
}

/// Properties for a text node.
#[derive(Debug, Clone, PartialEq)]
pub struct TextProps {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub font_size: f32,
    pub color: Color,
}

/// A retained scene node.
#[derive(Debug, Clone)]
pub enum SceneNode {
    Rect(RectProps),
    Text(TextProps),
}

/// Internal node entry with dirty tracking.
#[derive(Debug)]
struct NodeEntry {
    id: NodeId,
    node: SceneNode,
    dirty: bool,
    /// Generation counter — incremented on each update.
    generation: u64,
}

/// Retained scene graph container.
///
/// Owns all scene nodes and tracks which ones are dirty (need re-upload).
/// Thread safety: single-threaded, immediate-mutating API.
pub struct Scene {
    nodes: Vec<NodeEntry>,
    next_id: u32,
    /// Global dirty flag — set when any node changes.
    dirty: bool,
}

impl Scene {
    /// Create a new empty scene.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            next_id: 0,
            dirty: false,
        }
    }

    /// Add a rectangle node. Returns the node ID.
    pub fn add_rect(&mut self, props: RectProps) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        self.nodes.push(NodeEntry {
            id,
            node: SceneNode::Rect(props),
            dirty: true,
            generation: 0,
        });
        self.dirty = true;
        id
    }

    /// Add a text node. Returns the node ID.
    pub fn add_text(&mut self, props: TextProps) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        self.nodes.push(NodeEntry {
            id,
            node: SceneNode::Text(props),
            dirty: true,
            generation: 0,
        });
        self.dirty = true;
        id
    }

    /// Update an existing node. Marks it dirty.
    ///
    /// Returns `true` if the node was found and updated.
    pub fn update(&mut self, id: NodeId, node: SceneNode) -> bool {
        if let Some(entry) = self.nodes.iter_mut().find(|e| e.id == id) {
            entry.node = node;
            entry.dirty = true;
            entry.generation += 1;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// Remove a node by ID. Returns `true` if found.
    pub fn remove(&mut self, id: NodeId) -> bool {
        let len_before = self.nodes.len();
        self.nodes.retain(|e| e.id != id);
        let removed = self.nodes.len() < len_before;
        if removed {
            self.dirty = true;
        }
        removed
    }

    /// Check if any node is dirty (needs re-render).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Iterate over all nodes (read-only).
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &SceneNode)> {
        self.nodes.iter().map(|e| (e.id, &e.node))
    }

    /// Iterate over dirty nodes only (for incremental re-upload).
    pub fn dirty_nodes(&self) -> impl Iterator<Item = (NodeId, &SceneNode)> {
        self.nodes
            .iter()
            .filter(|e| e.dirty)
            .map(|e| (e.id, &e.node))
    }

    /// Clear all dirty flags after a successful render.
    pub fn clear_dirty(&mut self) {
        for entry in &mut self.nodes {
            entry.dirty = false;
        }
        self.dirty = false;
    }

    /// Number of nodes in the scene.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the scene is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_iterate() {
        let mut scene = Scene::new();
        let id1 = scene.add_rect(RectProps {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
            color: Color::WHITE,
        });
        let id2 = scene.add_text(TextProps {
            x: 10.0,
            y: 10.0,
            text: "Hello".into(),
            font_size: 16.0,
            color: Color::BLACK,
        });

        assert_eq!(scene.len(), 2);
        assert!(scene.is_dirty());

        let ids: Vec<NodeId> = scene.iter().map(|(id, _)| id).collect();
        assert_eq!(ids, vec![id1, id2]);
    }

    #[test]
    fn dirty_tracking() {
        let mut scene = Scene::new();
        let id = scene.add_rect(RectProps {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            color: Color::WHITE,
        });

        assert!(scene.is_dirty());
        assert_eq!(scene.dirty_nodes().count(), 1);

        scene.clear_dirty();
        assert!(!scene.is_dirty());
        assert_eq!(scene.dirty_nodes().count(), 0);

        // Update should re-mark dirty
        scene.update(
            id,
            SceneNode::Rect(RectProps {
                x: 5.0,
                y: 5.0,
                width: 10.0,
                height: 10.0,
                color: Color::BLACK,
            }),
        );
        assert!(scene.is_dirty());
        assert_eq!(scene.dirty_nodes().count(), 1);
    }

    #[test]
    fn remove_node() {
        let mut scene = Scene::new();
        let id = scene.add_rect(RectProps {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            color: Color::WHITE,
        });

        assert_eq!(scene.len(), 1);
        assert!(scene.remove(id));
        assert_eq!(scene.len(), 0);
        assert!(!scene.remove(id)); // already removed
    }

    #[test]
    fn steady_state_no_dirty() {
        let mut scene = Scene::new();
        scene.add_rect(RectProps {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            color: Color::TRANSPARENT,
        });
        scene.clear_dirty();

        // Steady state: no mutations = no dirty
        for _ in 0..100 {
            assert!(!scene.is_dirty());
        }
    }
}
