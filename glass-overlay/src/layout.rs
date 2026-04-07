//! Anchor-based layout system with flat-list widget management.
//!
//! Provides a minimal UI toolkit for positioning overlay modules on screen:
//!
//! - [`Widget`] trait — `bounding_box()`, `contains_point()`, `draw()`
//! - [`WidgetWrapper`] — composition wrapper that positions an [`OverlayModule`]
//!   via an [`Anchor`]; the wrapper manages position/size, the module manages content
//! - [`LayoutManager`] — flat list of positioned widgets, O(n) hit-testing
//!   (n ≤ 10), resize recalculation
//! - [`Anchor`] — anchor points for screen-relative positioning
//!
//! # Performance
//!
//! Hit-testing is a linear scan of the flat widget list. With < 10 widgets
//! this is effectively O(1). No tree traversal, no spatial indexing overhead.
//!
//! # Resize Resilience
//!
//! On `WM_SIZE` / `WM_DISPLAYCHANGE`, call [`LayoutManager::recalculate`] to
//! recompute all widget positions from their anchors. Modules whose position
//! changed are deinit + reinit to recreate scene nodes at the correct location.

use crate::modules::{self, ModuleInfo, ModulesConfig, OverlayModule};
use crate::scene::Scene;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tracing::debug;

// ─── Anchor ─────────────────────────────────────────────────────────────────

/// Anchor point defining how a widget is positioned relative to the screen.
///
/// The anchor determines which screen edge/corner the widget is aligned to.
/// A margin offset is applied from the anchor point inward.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Anchor {
    /// Top-left corner (default). Margin pushes right and down.
    #[default]
    TopLeft,
    /// Top-right corner. Margin pushes left and down.
    TopRight,
    /// Bottom-left corner. Margin pushes right and up.
    BottomLeft,
    /// Bottom-right corner. Margin pushes left and up.
    BottomRight,
    /// Center of the screen. Margin shifts from center.
    Center,
    /// Position as a percentage of screen dimensions.
    /// Values in `[0.0, 1.0]` — e.g. `(0.5, 0.5)` is screen center.
    ScreenPercentage(f32, f32),
}

impl Anchor {
    /// Resolve this anchor to absolute screen coordinates (top-left of widget).
    ///
    /// Returns `(x, y)` in screen pixels where the widget's top-left corner
    /// should be placed.
    pub fn resolve(
        &self,
        content_w: f32,
        content_h: f32,
        screen_w: f32,
        screen_h: f32,
        margin_x: f32,
        margin_y: f32,
    ) -> (f32, f32) {
        match self {
            Anchor::TopLeft => (margin_x, margin_y),
            Anchor::TopRight => (screen_w - content_w - margin_x, margin_y),
            Anchor::BottomLeft => (margin_x, screen_h - content_h - margin_y),
            Anchor::BottomRight => (
                screen_w - content_w - margin_x,
                screen_h - content_h - margin_y,
            ),
            Anchor::Center => (
                (screen_w - content_w) / 2.0 + margin_x,
                (screen_h - content_h) / 2.0 + margin_y,
            ),
            Anchor::ScreenPercentage(px, py) => {
                (screen_w * px + margin_x, screen_h * py + margin_y)
            }
        }
    }
}

// ─── BoundingBox ────────────────────────────────────────────────────────────

/// Axis-aligned bounding rectangle in screen coordinates.
///
/// Used for hit-testing and layout calculations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    /// Left edge in screen pixels.
    pub x: f32,
    /// Top edge in screen pixels.
    pub y: f32,
    /// Rectangle width in pixels.
    pub width: f32,
    /// Rectangle height in pixels.
    pub height: f32,
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self::ZERO
    }
}

impl BoundingBox {
    /// Zero-sized bounding box at the origin.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    /// Create a new bounding box.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Test whether a point `(px, py)` is inside this bounding box.
    ///
    /// Uses half-open interval: left/top inclusive, right/bottom exclusive.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

// ─── Widget Trait ───────────────────────────────────────────────────────────

/// Minimal UI widget abstraction.
///
/// Widgets have a bounding box for layout/hit-testing, can test point
/// containment, and can draw themselves into the scene graph.
pub trait Widget {
    /// Returns the widget's current bounding box in screen coordinates.
    fn bounding_box(&self) -> BoundingBox;

    /// Test whether a screen-space point is inside this widget.
    fn contains_point(&self, x: f32, y: f32) -> bool;

    /// Draw (or update) the widget's visual representation in the scene.
    fn draw(&mut self, scene: &mut Scene);
}

// ─── WidgetWrapper ──────────────────────────────────────────────────────────

/// Composition wrapper: positions an [`OverlayModule`] on screen via an [`Anchor`].
///
/// The wrapper manages **position and size**; the inner module manages **content**.
/// On layout recalculation (e.g. resize), the wrapper computes absolute
/// coordinates from the anchor and calls `set_position` on the module.
///
/// # Zero-Intrusion
///
/// Modules are not rewritten — they are encapsulated. The wrapper translates
/// anchor-based positioning into concrete `(x, y)` coordinates that the
/// module uses for its scene nodes.
pub struct WidgetWrapper<M: OverlayModule> {
    module: M,
    anchor: Anchor,
    margin: (f32, f32),
    bbox: BoundingBox,
    screen_size: (f32, f32),
}

impl<M> fmt::Debug for WidgetWrapper<M>
where
    M: OverlayModule + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WidgetWrapper")
            .field("module", &self.module)
            .field("anchor", &self.anchor)
            .field("margin", &self.margin)
            .field("bbox", &self.bbox)
            .field("screen_size", &self.screen_size)
            .finish()
    }
}

impl<M: OverlayModule> WidgetWrapper<M> {
    /// Create a new widget wrapper.
    ///
    /// # Arguments
    /// * `module` — the overlay module to wrap
    /// * `anchor` — how to position the widget relative to the screen
    /// * `margin_x`, `margin_y` — offset from the anchor edge in pixels
    pub fn new(module: M, anchor: Anchor, margin_x: f32, margin_y: f32) -> Self {
        Self {
            module,
            anchor,
            margin: (margin_x, margin_y),
            bbox: BoundingBox::ZERO,
            screen_size: (0.0, 0.0),
        }
    }

    /// Recalculate the widget's position for the given screen dimensions.
    ///
    /// Calls `set_position` on the inner module to propagate the new coordinates.
    pub fn recalculate(&mut self, screen_w: f32, screen_h: f32) {
        self.screen_size = (screen_w, screen_h);
        let (cw, ch) = self.module.content_size();
        let (x, y) = self
            .anchor
            .resolve(cw, ch, screen_w, screen_h, self.margin.0, self.margin.1);
        self.bbox = BoundingBox::new(x, y, cw, ch);
        self.module.set_position(x, y);
    }

    /// Get a reference to the inner module.
    pub fn module(&self) -> &M {
        &self.module
    }

    /// Get a mutable reference to the inner module.
    pub fn module_mut(&mut self) -> &mut M {
        &mut self.module
    }

    /// Get the current anchor.
    pub fn anchor(&self) -> &Anchor {
        &self.anchor
    }

    /// Set a new anchor and recalculate position if screen size is known.
    pub fn set_anchor(&mut self, anchor: Anchor) {
        self.anchor = anchor;
        if self.screen_size.0 > 0.0 {
            self.recalculate(self.screen_size.0, self.screen_size.1);
        }
    }
}

impl<M: OverlayModule> Widget for WidgetWrapper<M> {
    fn bounding_box(&self) -> BoundingBox {
        self.bbox
    }

    fn contains_point(&self, x: f32, y: f32) -> bool {
        self.bbox.contains(x, y)
    }

    fn draw(&mut self, scene: &mut Scene) {
        if self.module.enabled() {
            self.module.update(scene, Duration::ZERO);
        }
    }
}

// ─── Layout Configuration ──────────────────────────────────────────────────

/// Per-widget layout configuration (serialized in the config file).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WidgetLayoutConfig {
    /// Anchor point for positioning.
    #[serde(default)]
    pub anchor: Anchor,
    /// Horizontal margin from the anchor edge (pixels).
    #[serde(default = "default_margin")]
    pub margin_x: f32,
    /// Vertical margin from the anchor edge (pixels).
    #[serde(default = "default_margin")]
    pub margin_y: f32,
}

fn default_margin() -> f32 {
    10.0
}

impl Default for WidgetLayoutConfig {
    fn default() -> Self {
        Self {
            anchor: Anchor::TopLeft,
            margin_x: 10.0,
            margin_y: 10.0,
        }
    }
}

/// Layout configuration for all built-in modules.
///
/// Defaults match the legacy hardcoded positions (top-left stacked).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// Clock widget layout.
    #[serde(default)]
    pub clock: WidgetLayoutConfig,
    /// System stats widget layout.
    #[serde(default = "default_stats_layout")]
    pub system_stats: WidgetLayoutConfig,
    /// FPS counter widget layout.
    #[serde(default = "default_fps_layout")]
    pub fps: WidgetLayoutConfig,
}

fn default_stats_layout() -> WidgetLayoutConfig {
    WidgetLayoutConfig {
        anchor: Anchor::TopLeft,
        margin_x: 10.0,
        margin_y: 34.0,
    }
}

fn default_fps_layout() -> WidgetLayoutConfig {
    WidgetLayoutConfig {
        anchor: Anchor::TopLeft,
        margin_x: 10.0,
        margin_y: 60.0,
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            clock: WidgetLayoutConfig::default(),
            system_stats: default_stats_layout(),
            fps: default_fps_layout(),
        }
    }
}

// ─── LayoutManager ──────────────────────────────────────────────────────────

/// Internal entry storing a type-erased module with its layout metadata.
struct LayoutEntry {
    module: Box<dyn OverlayModule>,
    anchor: Anchor,
    margin: (f32, f32),
    bbox: BoundingBox,
}

/// Flat list of positioned widgets with module lifecycle management.
///
/// Replaces direct [`ModuleRegistry`](crate::modules::ModuleRegistry) usage
/// when anchor-based layout is needed. Provides the same lifecycle API
/// (init/update/deinit/set_enabled/apply_config) plus layout management.
///
/// # Hit-Testing Performance
///
/// Linear scan over the flat widget list. With fewer than 10 widgets
/// (typical overlay), this is effectively constant-time.
///
/// # Resize Resilience
///
/// Call [`recalculate`](Self::recalculate) on `WM_SIZE` / `WM_DISPLAYCHANGE`.
/// Widgets whose position changed are deinit + reinit to recreate scene
/// nodes at the correct location.
pub struct LayoutManager {
    entries: Vec<LayoutEntry>,
    screen_w: f32,
    screen_h: f32,
}

impl fmt::Debug for LayoutManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let module_ids: Vec<_> = self
            .entries
            .iter()
            .map(|entry| entry.module.info().id)
            .collect();
        f.debug_struct("LayoutManager")
            .field("screen_w", &self.screen_w)
            .field("screen_h", &self.screen_h)
            .field("module_ids", &module_ids)
            .finish()
    }
}

impl LayoutManager {
    /// Create a new layout manager with the given screen dimensions.
    pub fn new(screen_w: f32, screen_h: f32) -> Self {
        Self {
            entries: Vec::new(),
            screen_w,
            screen_h,
        }
    }

    /// Add a widget wrapper to the layout. Computes initial position from anchor.
    ///
    /// Consumes the [`WidgetWrapper`], type-erasing the module for heterogeneous
    /// storage. The anchor and margin are preserved for resize recalculation.
    ///
    /// # Panics
    /// Panics if a module with the same ID already exists.
    pub fn add_widget<M: OverlayModule + 'static>(&mut self, wrapper: WidgetWrapper<M>) {
        let WidgetWrapper {
            mut module,
            anchor,
            margin,
            ..
        } = wrapper;

        let id = module.info().id;
        assert!(
            !self.entries.iter().any(|e| e.module.info().id == id),
            "Duplicate module ID in layout: {id}"
        );

        let (cw, ch) = module.content_size();
        let (x, y) = anchor.resolve(cw, ch, self.screen_w, self.screen_h, margin.0, margin.1);
        module.set_position(x, y);
        let bbox = BoundingBox::new(x, y, cw, ch);

        debug!(
            "Layout: added '{}' at ({:.0}, {:.0}) {:.0}x{:.0}, anchor={:?}",
            id, x, y, cw, ch, anchor
        );

        self.entries.push(LayoutEntry {
            module: Box::new(module),
            anchor,
            margin,
            bbox,
        });
    }

    /// Recalculate all widget positions for new screen dimensions.
    ///
    /// Modules whose position changed are deinit + reinit to recreate
    /// scene nodes at the correct location. Only triggers on actual change.
    ///
    /// Call this on `WM_SIZE` / `WM_DISPLAYCHANGE`.
    pub fn recalculate(&mut self, screen_w: f32, screen_h: f32, scene: &mut Scene) {
        self.screen_w = screen_w;
        self.screen_h = screen_h;

        for entry in &mut self.entries {
            let (cw, ch) = entry.module.content_size();
            let (x, y) =
                entry
                    .anchor
                    .resolve(cw, ch, screen_w, screen_h, entry.margin.0, entry.margin.1);
            let new_bbox = BoundingBox::new(x, y, cw, ch);

            if new_bbox != entry.bbox {
                entry.bbox = new_bbox;
                entry.module.set_position(x, y);

                // Recreate scene nodes at new position
                if entry.module.enabled() {
                    entry.module.deinit(scene);
                    entry.module.init(scene);
                }
            }
        }

        debug!("Layout: recalculated for {screen_w:.0}x{screen_h:.0}");
    }

    /// Hit-test a point against all enabled widgets.
    ///
    /// Returns the module ID of the first hit, or `None`.
    /// Linear scan — O(n) where n is the widget count (typically < 10).
    pub fn hit_test(&self, x: f32, y: f32) -> Option<&'static str> {
        self.entries
            .iter()
            .filter(|e| e.module.enabled())
            .find(|e| e.bbox.contains(x, y))
            .map(|e| e.module.info().id)
    }

    /// Get the current screen dimensions.
    pub fn screen_size(&self) -> (f32, f32) {
        (self.screen_w, self.screen_h)
    }

    // ── Module lifecycle delegation ─────────────────────────────────────

    /// Initialize all enabled modules (add scene nodes at computed positions).
    pub fn init_all(&mut self, scene: &mut Scene) {
        for entry in &mut self.entries {
            if entry.module.enabled() {
                entry.module.init(scene);
            }
        }
    }

    /// Update all enabled modules. Returns `true` if any modified the scene.
    pub fn update_all(&mut self, scene: &mut Scene, dt: Duration) -> bool {
        let mut dirty = false;
        for entry in &mut self.entries {
            if entry.module.enabled() {
                dirty |= entry.module.update(scene, dt);
            }
        }
        dirty
    }

    /// Deinitialize all modules (remove scene nodes).
    pub fn deinit_all(&mut self, scene: &mut Scene) {
        for entry in &mut self.entries {
            entry.module.deinit(scene);
        }
    }

    /// Enable or disable a module by ID.
    ///
    /// If enabling, calls `init`. If disabling, calls `deinit`.
    /// Returns `true` if the module was found.
    pub fn set_enabled(&mut self, id: &str, enabled: bool, scene: &mut Scene) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.module.info().id == id) {
            let was_enabled = entry.module.enabled();
            entry.module.set_enabled(enabled);
            if enabled && !was_enabled {
                entry.module.init(scene);
            } else if !enabled && was_enabled {
                entry.module.deinit(scene);
            }
            true
        } else {
            false
        }
    }

    /// Number of managed widgets.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the manager has no widgets.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// List all module infos with their enabled state.
    pub fn list(&self) -> Vec<(ModuleInfo, bool)> {
        self.entries
            .iter()
            .map(|e| (e.module.info(), e.module.enabled()))
            .collect()
    }

    /// Apply modules config (enable/disable + per-module settings).
    pub fn apply_config(&mut self, config: &ModulesConfig, scene: &mut Scene) {
        self.set_enabled("clock", config.clock_enabled, scene);
        self.set_enabled("system_stats", config.system_stats_enabled, scene);
        self.set_enabled("fps", config.fps_enabled, scene);

        for entry in &mut self.entries {
            if entry.module.info().id == "clock" {
                if let Some(clock) = entry
                    .module
                    .as_any_mut()
                    .downcast_mut::<modules::clock::ClockModule>()
                {
                    clock.set_format(&config.clock_format);
                }
            }
            if entry.module.info().id == "system_stats" {
                if let Some(stats) = entry
                    .module
                    .as_any_mut()
                    .downcast_mut::<modules::system_stats::SystemStatsModule>()
                {
                    stats.set_interval(Duration::from_millis(config.stats_interval_ms));
                }
            }
        }
    }
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Color, NodeId, Scene, TextProps};

    // ── Test helper module ──────────────────────────────────────────────

    struct TestModule {
        id: &'static str,
        enabled: bool,
        node_ids: Vec<NodeId>,
        pos: (f32, f32),
        size: (f32, f32),
    }

    impl TestModule {
        fn new(id: &'static str, w: f32, h: f32) -> Self {
            Self {
                id,
                enabled: true,
                node_ids: Vec::new(),
                pos: (0.0, 0.0),
                size: (w, h),
            }
        }
    }

    impl OverlayModule for TestModule {
        fn info(&self) -> ModuleInfo {
            ModuleInfo {
                id: self.id,
                name: self.id,
                description: "test",
            }
        }

        fn init(&mut self, scene: &mut Scene) {
            let id = scene.add_text(TextProps {
                x: self.pos.0,
                y: self.pos.1,
                text: format!("{}@({:.0},{:.0})", self.id, self.pos.0, self.pos.1),
                font_size: 14.0,
                color: Color::WHITE,
            });
            self.node_ids.push(id);
        }

        fn update(&mut self, _scene: &mut Scene, _dt: Duration) -> bool {
            false
        }

        fn deinit(&mut self, scene: &mut Scene) {
            for id in self.node_ids.drain(..) {
                scene.remove(id);
            }
        }

        fn enabled(&self) -> bool {
            self.enabled
        }

        fn set_enabled(&mut self, enabled: bool) {
            self.enabled = enabled;
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn set_position(&mut self, x: f32, y: f32) {
            self.pos = (x, y);
        }

        fn content_size(&self) -> (f32, f32) {
            self.size
        }
    }

    // ── Anchor tests ────────────────────────────────────────────────────

    #[test]
    fn anchor_top_left() {
        let (x, y) = Anchor::TopLeft.resolve(100.0, 50.0, 1920.0, 1080.0, 10.0, 20.0);
        assert_eq!((x, y), (10.0, 20.0));
    }

    #[test]
    fn anchor_top_right() {
        let (x, y) = Anchor::TopRight.resolve(100.0, 50.0, 1920.0, 1080.0, 10.0, 20.0);
        assert_eq!((x, y), (1810.0, 20.0)); // 1920 - 100 - 10
    }

    #[test]
    fn anchor_bottom_left() {
        let (x, y) = Anchor::BottomLeft.resolve(100.0, 50.0, 1920.0, 1080.0, 10.0, 20.0);
        assert_eq!((x, y), (10.0, 1010.0)); // 1080 - 50 - 20
    }

    #[test]
    fn anchor_bottom_right() {
        let (x, y) = Anchor::BottomRight.resolve(100.0, 50.0, 1920.0, 1080.0, 10.0, 20.0);
        assert_eq!((x, y), (1810.0, 1010.0));
    }

    #[test]
    fn anchor_center() {
        let (x, y) = Anchor::Center.resolve(100.0, 50.0, 1920.0, 1080.0, 0.0, 0.0);
        assert_eq!((x, y), (910.0, 515.0)); // (1920-100)/2, (1080-50)/2
    }

    #[test]
    fn anchor_center_with_margin() {
        let (x, y) = Anchor::Center.resolve(100.0, 50.0, 1920.0, 1080.0, 5.0, -10.0);
        assert_eq!((x, y), (915.0, 505.0)); // center + margin offset
    }

    #[test]
    fn anchor_screen_percentage() {
        let (x, y) =
            Anchor::ScreenPercentage(0.5, 0.25).resolve(100.0, 50.0, 1920.0, 1080.0, 0.0, 0.0);
        assert_eq!((x, y), (960.0, 270.0)); // 50%, 25%
    }

    #[test]
    fn anchor_screen_percentage_with_margin() {
        let (x, y) =
            Anchor::ScreenPercentage(0.0, 0.0).resolve(100.0, 50.0, 1920.0, 1080.0, 15.0, 25.0);
        assert_eq!((x, y), (15.0, 25.0));
    }

    // ── BoundingBox tests ───────────────────────────────────────────────

    #[test]
    fn bbox_contains_inside() {
        let bbox = BoundingBox::new(10.0, 20.0, 100.0, 50.0);
        assert!(bbox.contains(10.0, 20.0)); // top-left corner
        assert!(bbox.contains(50.0, 40.0)); // middle
        assert!(bbox.contains(109.9, 69.9)); // near bottom-right
    }

    #[test]
    fn bbox_contains_outside() {
        let bbox = BoundingBox::new(10.0, 20.0, 100.0, 50.0);
        assert!(!bbox.contains(9.9, 20.0)); // just left
        assert!(!bbox.contains(110.0, 20.0)); // right edge (exclusive)
        assert!(!bbox.contains(50.0, 70.0)); // bottom edge (exclusive)
        assert!(!bbox.contains(0.0, 0.0)); // origin
    }

    #[test]
    fn bbox_zero_size_never_contains() {
        let bbox = BoundingBox::ZERO;
        assert!(!bbox.contains(0.0, 0.0));
    }

    // ── WidgetWrapper tests ─────────────────────────────────────────────

    #[test]
    fn wrapper_recalculate_updates_position() {
        let module = TestModule::new("test", 100.0, 30.0);
        let mut wrapper = WidgetWrapper::new(module, Anchor::TopRight, 15.0, 10.0);

        wrapper.recalculate(1920.0, 1080.0);

        let bbox = wrapper.bounding_box();
        assert_eq!(bbox.x, 1805.0); // 1920 - 100 - 15
        assert_eq!(bbox.y, 10.0);
        assert_eq!(bbox.width, 100.0);
        assert_eq!(bbox.height, 30.0);
        assert_eq!(wrapper.module().pos, (1805.0, 10.0));
    }

    #[test]
    fn wrapper_contains_point_delegates_to_bbox() {
        let module = TestModule::new("test", 100.0, 50.0);
        let mut wrapper = WidgetWrapper::new(module, Anchor::TopLeft, 0.0, 0.0);
        wrapper.recalculate(1920.0, 1080.0);

        assert!(wrapper.contains_point(50.0, 25.0));
        assert!(!wrapper.contains_point(150.0, 25.0));
    }

    #[test]
    fn wrapper_set_anchor_recalculates() {
        let module = TestModule::new("test", 100.0, 50.0);
        let mut wrapper = WidgetWrapper::new(module, Anchor::TopLeft, 10.0, 10.0);
        wrapper.recalculate(1920.0, 1080.0);

        assert_eq!(wrapper.bounding_box().x, 10.0);

        wrapper.set_anchor(Anchor::TopRight);
        assert_eq!(wrapper.bounding_box().x, 1810.0); // 1920 - 100 - 10
    }

    // ── LayoutManager tests ─────────────────────────────────────────────

    #[test]
    fn layout_add_and_hit_test() {
        let mut lm = LayoutManager::new(1920.0, 1080.0);

        let m1 = TestModule::new("w1", 100.0, 30.0);
        lm.add_widget(WidgetWrapper::new(m1, Anchor::TopLeft, 10.0, 10.0));

        let m2 = TestModule::new("w2", 100.0, 30.0);
        lm.add_widget(WidgetWrapper::new(m2, Anchor::TopRight, 10.0, 10.0));

        assert_eq!(lm.len(), 2);

        // Hit w1 (top-left)
        assert_eq!(lm.hit_test(15.0, 15.0), Some("w1"));
        // Hit w2 (top-right)
        assert_eq!(lm.hit_test(1815.0, 15.0), Some("w2"));
        // Miss
        assert_eq!(lm.hit_test(960.0, 540.0), None);
    }

    #[test]
    fn layout_hit_test_skips_disabled() {
        let mut lm = LayoutManager::new(1920.0, 1080.0);
        let mut scene = Scene::new();

        let mut m = TestModule::new("disabled_w", 100.0, 30.0);
        m.enabled = false;
        lm.add_widget(WidgetWrapper::new(m, Anchor::TopLeft, 10.0, 10.0));

        // Module is at (10, 10) but disabled — should not hit
        assert_eq!(lm.hit_test(15.0, 15.0), None);

        // Enable it
        lm.set_enabled("disabled_w", true, &mut scene);
        assert_eq!(lm.hit_test(15.0, 15.0), Some("disabled_w"));
    }

    #[test]
    fn layout_recalculate_repositions() {
        let mut lm = LayoutManager::new(1920.0, 1080.0);
        let mut scene = Scene::new();

        let m = TestModule::new("corner", 100.0, 50.0);
        lm.add_widget(WidgetWrapper::new(m, Anchor::BottomRight, 10.0, 10.0));
        lm.init_all(&mut scene);

        // Initial position: bottom-right of 1920x1080
        assert_eq!(lm.hit_test(1815.0, 1025.0), Some("corner"));

        // Resize to 1280x720
        lm.recalculate(1280.0, 720.0, &mut scene);

        // Old position should miss
        assert_eq!(lm.hit_test(1815.0, 1025.0), None);
        // New position: 1280-100-10=1170, 720-50-10=660
        assert_eq!(lm.hit_test(1175.0, 665.0), Some("corner"));
    }

    #[test]
    fn layout_init_deinit_lifecycle() {
        let mut lm = LayoutManager::new(1920.0, 1080.0);
        let mut scene = Scene::new();

        lm.add_widget(WidgetWrapper::new(
            TestModule::new("lc", 100.0, 30.0),
            Anchor::TopLeft,
            0.0,
            0.0,
        ));

        assert_eq!(scene.len(), 0);
        lm.init_all(&mut scene);
        assert_eq!(scene.len(), 1);

        lm.deinit_all(&mut scene);
        assert_eq!(scene.len(), 0);
    }

    #[test]
    #[should_panic(expected = "Duplicate module ID")]
    fn layout_duplicate_panics() {
        let mut lm = LayoutManager::new(1920.0, 1080.0);
        lm.add_widget(WidgetWrapper::new(
            TestModule::new("dup", 10.0, 10.0),
            Anchor::TopLeft,
            0.0,
            0.0,
        ));
        lm.add_widget(WidgetWrapper::new(
            TestModule::new("dup", 10.0, 10.0),
            Anchor::TopRight,
            0.0,
            0.0,
        ));
    }

    // ── Config tests ────────────────────────────────────────────────────

    #[test]
    fn layout_config_defaults_match_legacy() {
        let cfg = LayoutConfig::default();
        // Clock: top-left at (10, 10) — same as legacy DEFAULT_X/DEFAULT_Y
        assert_eq!(cfg.clock.anchor, Anchor::TopLeft);
        assert_eq!(cfg.clock.margin_x, 10.0);
        assert_eq!(cfg.clock.margin_y, 10.0);
        // Stats: top-left at (10, 34)
        assert_eq!(cfg.system_stats.margin_y, 34.0);
        // FPS: top-left at (10, 60)
        assert_eq!(cfg.fps.margin_y, 60.0);
    }

    #[test]
    fn layout_config_ron_roundtrip() {
        let cfg = LayoutConfig::default();
        let ron_str = ron::to_string(&cfg).unwrap();
        let parsed: LayoutConfig = ron::from_str(&ron_str).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn anchor_ron_roundtrip() {
        let anchors = vec![
            Anchor::TopLeft,
            Anchor::TopRight,
            Anchor::BottomLeft,
            Anchor::BottomRight,
            Anchor::Center,
            Anchor::ScreenPercentage(0.5, 0.25),
        ];
        for anchor in anchors {
            let s = ron::to_string(&anchor).unwrap();
            let parsed: Anchor = ron::from_str(&s).unwrap();
            assert_eq!(anchor, parsed);
        }
    }

    // ── apply_config integration ─────────────────────────────────────────

    #[test]
    fn apply_config_disables_module_by_id() {
        let mut lm = LayoutManager::new(1920.0, 1080.0);
        let mut scene = Scene::new();

        // Register a module whose ID matches a key apply_config controls
        let m = TestModule::new("clock", 150.0, 22.0);
        lm.add_widget(WidgetWrapper::new(m, Anchor::TopLeft, 10.0, 10.0));
        lm.init_all(&mut scene);
        assert_eq!(scene.len(), 1, "clock module should have added one node");

        // Disable clock via apply_config
        let mut cfg = ModulesConfig::default();
        cfg.clock_enabled = false;
        lm.apply_config(&cfg, &mut scene);

        assert_eq!(
            scene.len(),
            0,
            "disabling clock via apply_config should remove its scene node"
        );
        assert_eq!(
            lm.hit_test(15.0, 15.0),
            None,
            "disabled module must not be hit-testable"
        );
    }

    #[test]
    fn apply_config_enables_module_by_id() {
        let mut lm = LayoutManager::new(1920.0, 1080.0);
        let mut scene = Scene::new();

        let mut m = TestModule::new("clock", 150.0, 22.0);
        m.enabled = false; // Start disabled
        lm.add_widget(WidgetWrapper::new(m, Anchor::TopLeft, 10.0, 10.0));
        assert_eq!(
            scene.len(),
            0,
            "disabled module must not add nodes before init"
        );

        // Enable clock via apply_config (clock_enabled defaults to true)
        let cfg = ModulesConfig::default(); // clock_enabled = true
        lm.apply_config(&cfg, &mut scene);

        assert_eq!(
            scene.len(),
            1,
            "enabling clock via apply_config should add its scene node"
        );
    }
}
