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
    /// Red channel in `[0.0, 1.0]`.
    pub r: f32,
    /// Green channel in `[0.0, 1.0]`.
    pub g: f32,
    /// Blue channel in `[0.0, 1.0]`.
    pub b: f32,
    /// Alpha channel in `[0.0, 1.0]`. For premultiplied colours, RGB values
    /// should already be multiplied by alpha before they are stored here.
    pub a: f32,
}

impl Color {
    /// Construct a colour from individual RGBA components (all in `[0.0, 1.0]`).
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

    /// Fully transparent black (all components zero).
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);
    /// Opaque white.
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0, 1.0);
    /// Opaque black.
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0, 1.0);
}

/// Properties for a rectangle node.
#[derive(Debug, Clone, PartialEq)]
pub struct RectProps {
    /// Horizontal position of the rectangle's top-left corner in screen pixels.
    pub x: f32,
    /// Vertical position of the rectangle's top-left corner in screen pixels.
    pub y: f32,
    /// Rectangle width in pixels.
    pub width: f32,
    /// Rectangle height in pixels.
    pub height: f32,
    /// Fill colour (premultiplied alpha).
    pub color: Color,
}

/// Properties for a text node.
#[derive(Debug, Clone, PartialEq)]
pub struct TextProps {
    /// Horizontal position of the text baseline origin in screen pixels.
    pub x: f32,
    /// Vertical position of the text baseline origin in screen pixels.
    pub y: f32,
    /// Text content to render.
    pub text: String,
    /// Font size in logical pixels.
    pub font_size: f32,
    /// Text colour (premultiplied alpha).
    pub color: Color,
}

/// A retained scene node.
#[derive(Debug, Clone)]
pub enum SceneNode {
    /// A solid-colour rectangle.
    Rect(RectProps),
    /// A text run rendered via glyphon.
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

    // ── NodeId uniqueness ────────────────────────────────────────────────

    #[test]
    fn node_ids_are_unique_across_multiple_adds() {
        let mut scene = Scene::new();
        let id1 = scene.add_text(TextProps {
            x: 0.0,
            y: 0.0,
            text: "a".into(),
            font_size: 12.0,
            color: Color::WHITE,
        });
        let id2 = scene.add_text(TextProps {
            x: 1.0,
            y: 0.0,
            text: "b".into(),
            font_size: 12.0,
            color: Color::WHITE,
        });
        let id3 = scene.add_rect(RectProps {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            color: Color::BLACK,
        });
        assert_ne!(id1, id2, "consecutive text nodes must have distinct IDs");
        assert_ne!(id2, id3, "text and rect nodes must have distinct IDs");
        assert_ne!(id1, id3, "non-adjacent nodes must have distinct IDs");
    }

    // ── update return value ──────────────────────────────────────────────

    #[test]
    fn update_existing_node_returns_true() {
        let mut scene = Scene::new();
        let id = scene.add_text(TextProps {
            x: 0.0,
            y: 0.0,
            text: "original".into(),
            font_size: 12.0,
            color: Color::WHITE,
        });
        let updated = scene.update(
            id,
            SceneNode::Text(TextProps {
                x: 5.0,
                y: 5.0,
                text: "updated".into(),
                font_size: 14.0,
                color: Color::BLACK,
            }),
        );
        assert!(updated, "update on existing node should return true");
    }

    #[test]
    fn update_nonexistent_node_returns_false() {
        let mut scene = Scene::new();
        let fake_id = NodeId(999);
        let result = scene.update(
            fake_id,
            SceneNode::Text(TextProps {
                x: 0.0,
                y: 0.0,
                text: "ghost".into(),
                font_size: 12.0,
                color: Color::WHITE,
            }),
        );
        assert!(!result, "update on nonexistent node should return false");
        // Scene should remain empty and clean
        assert!(scene.is_empty());
    }

    // ── empty-scene edge cases ───────────────────────────────────────────

    #[test]
    fn new_scene_is_empty_and_clean() {
        let scene = Scene::new();
        assert!(scene.is_empty());
        assert_eq!(scene.len(), 0);
        assert!(!scene.is_dirty(), "fresh scene must not be dirty");
        assert_eq!(scene.iter().count(), 0);
    }

    #[test]
    fn remove_on_empty_scene_returns_false() {
        let mut scene = Scene::new();
        assert!(!scene.remove(NodeId(0)), "remove on empty scene should return false");
        assert!(scene.is_empty());
    }

    #[test]
    fn remove_all_nodes_leaves_empty_scene() {
        let mut scene = Scene::new();
        let id1 = scene.add_text(TextProps {
            x: 0.0,
            y: 0.0,
            text: "x".into(),
            font_size: 12.0,
            color: Color::WHITE,
        });
        let id2 = scene.add_rect(RectProps {
            x: 0.0,
            y: 0.0,
            width: 5.0,
            height: 5.0,
            color: Color::BLACK,
        });
        assert_eq!(scene.len(), 2);
        scene.remove(id1);
        scene.remove(id2);
        assert!(scene.is_empty(), "scene should be empty after removing all nodes");
        assert_eq!(scene.len(), 0);
    }
}
