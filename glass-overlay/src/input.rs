//! Input mode subsystem: passive/interactive mode switching and rect-based hit-testing.
//!
//! The overlay supports two input modes:
//! - **Passive (Mode A)** — default; the overlay is fully click-through.
//! - **Interactive (Mode B)** — triggered by a global hotkey; mouse input is
//!   accepted on designated interactive rectangles for a configurable timeout.
//!
//! # Architecture
//!
//! [`OverlayInputState`] is stored in the HWND's `GWLP_USERDATA` and accessed
//! from the `wnd_proc` (single-threaded). [`HitTester`] performs rectangle-based
//! hit-testing with Z-order support. [`InputManager`] drives mode transitions
//! and visual indicator lifecycle.

use std::time::{Duration, Instant};
use tracing::{debug, info};

// ─── Custom Windows messages ────────────────────────────────────────────────
/// Posted when input mode transitions to interactive.
pub const WM_GLASS_MODE_INTERACTIVE: u32 = 0x8000 + 10; // WM_APP + 10
/// Posted when input mode transitions back to passive.
pub const WM_GLASS_MODE_PASSIVE: u32 = 0x8000 + 11; // WM_APP + 11
/// Win32 timer ID for the interactive-mode timeout.
pub const INTERACTIVE_TIMER_ID: usize = 42;
/// Win32 hotkey ID for the toggle hotkey.
pub const HOTKEY_ID: i32 = 1;

// ─── InputMode ──────────────────────────────────────────────────────────────

/// Current input mode of the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Fully click-through — no mouse events reach the overlay.
    Passive,
    /// Interactive — designated rects accept mouse input until timeout.
    Interactive,
}

impl std::fmt::Display for InputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputMode::Passive => write!(f, "Passive"),
            InputMode::Interactive => write!(f, "Interactive"),
        }
    }
}

// ─── InteractiveRect ────────────────────────────────────────────────────────

/// A named rectangular region that can receive mouse input in interactive mode.
///
/// Higher `z_order` values are tested first (topmost wins).
#[derive(Debug, Clone, PartialEq)]
pub struct InteractiveRect {
    /// Unique identifier.
    pub id: u32,
    /// Left edge in logical (DPI-independent) pixels.
    pub x: f32,
    /// Top edge in logical pixels.
    pub y: f32,
    /// Width in logical pixels.
    pub width: f32,
    /// Height in logical pixels.
    pub height: f32,
    /// Z-order: higher values are tested first.
    pub z_order: i32,
}

impl InteractiveRect {
    /// Test whether a point (px, py) is inside this rectangle.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x
            && px < self.x + self.width
            && py >= self.y
            && py < self.y + self.height
    }
}

// ─── HitTester ──────────────────────────────────────────────────────────────

/// Performs rectangle-based hit-testing with Z-order support.
///
/// Interactive UI nodes register their bounds here. During interactive mode,
/// mouse coordinates are tested against all registered rects, and the
/// topmost (highest `z_order`) hit is returned.
///
/// # Zero-allocation steady-state
/// The rect list is pre-allocated; hit-testing does not allocate.
#[derive(Debug, Default)]
pub struct HitTester {
    rects: Vec<InteractiveRect>,
    next_id: u32,
}

impl HitTester {
    /// Create an empty hit-tester.
    pub fn new() -> Self {
        Self {
            rects: Vec::new(),
            next_id: 0,
        }
    }

    /// Register an interactive rectangle. Returns its ID.
    pub fn add_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        z_order: i32,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.rects.push(InteractiveRect {
            id,
            x,
            y,
            width,
            height,
            z_order,
        });
        // Keep sorted by z_order descending for fast hit-test
        self.rects.sort_by(|a, b| b.z_order.cmp(&a.z_order));
        debug!("Interactive rect added: id={id}, ({x},{y} {width}x{height}), z={z_order}");
        id
    }

    /// Remove a registered rectangle by ID. Returns `true` if found.
    pub fn remove_rect(&mut self, id: u32) -> bool {
        let before = self.rects.len();
        self.rects.retain(|r| r.id != id);
        let removed = self.rects.len() < before;
        if removed {
            debug!("Interactive rect removed: id={id}");
        }
        removed
    }

    /// Hit-test a point against all registered rects.
    ///
    /// Returns the ID of the topmost (highest `z_order`) rect that contains
    /// the point, or `None` if no rect is hit.
    pub fn hit_test(&self, px: f32, py: f32) -> Option<u32> {
        // Rects are sorted by z_order descending, so the first match is topmost.
        self.rects
            .iter()
            .find(|r| r.contains(px, py))
            .map(|r| r.id)
    }

    /// Remove all registered rects.
    pub fn clear(&mut self) {
        self.rects.clear();
        debug!("All interactive rects cleared");
    }

    /// Number of registered rects.
    pub fn len(&self) -> usize {
        self.rects.len()
    }

    /// Whether no rects are registered.
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }
}

// ─── OverlayInputState ─────────────────────────────────────────────────────

/// Shared state stored in the HWND's `GWLP_USERDATA`.
///
/// Accessed exclusively from the window-proc thread (single-threaded by
/// the Win32 message-pump model).
pub struct OverlayInputState {
    /// Current input mode.
    pub mode: InputMode,
    /// Hit-tester for interactive regions.
    pub hit_tester: HitTester,
    /// Timeout duration for interactive mode.
    pub timeout: Duration,
    /// When interactive mode started (for diagnostics).
    pub interactive_since: Option<Instant>,
    /// Whether interactive mode is available (hotkey registered successfully).
    pub interactivity_available: bool,
}

impl OverlayInputState {
    /// Create a new input state in passive mode.
    pub fn new(timeout_ms: u32) -> Self {
        Self {
            mode: InputMode::Passive,
            hit_tester: HitTester::new(),
            timeout: Duration::from_millis(timeout_ms as u64),
            interactive_since: None,
            interactivity_available: true,
        }
    }

    /// Transition to interactive mode.
    ///
    /// Returns `true` if the mode actually changed (was passive),
    /// `false` if already interactive (timer reset only).
    pub fn enter_interactive(&mut self) -> bool {
        let was_passive = self.mode == InputMode::Passive;
        self.mode = InputMode::Interactive;
        self.interactive_since = Some(Instant::now());
        if was_passive {
            info!("Input mode: Passive → Interactive (timeout={}ms)", self.timeout.as_millis());
        } else {
            debug!("Interactive mode timer reset");
        }
        was_passive
    }

    /// Transition to passive mode.
    ///
    /// Returns `true` if the mode actually changed.
    pub fn enter_passive(&mut self) -> bool {
        let was_interactive = self.mode == InputMode::Interactive;
        self.mode = InputMode::Passive;
        self.interactive_since = None;
        if was_interactive {
            info!("Input mode: Interactive → Passive");
        }
        was_interactive
    }

    /// Whether the overlay is in interactive mode.
    pub fn is_interactive(&self) -> bool {
        self.mode == InputMode::Interactive
    }
}

// ─── InputManager (high-level API) ──────────────────────────────────────────

/// High-level manager for input mode transitions and indicator lifecycle.
///
/// Used by the main application loop (not the wnd_proc directly).
pub struct InputManager {
    /// Scene node IDs for the visual indicator (border rects + label).
    indicator_node_ids: Vec<crate::scene::NodeId>,
    /// Whether the indicator is currently visible.
    indicator_visible: bool,
}

impl InputManager {
    /// Create a new InputManager.
    pub fn new() -> Self {
        Self {
            indicator_node_ids: Vec::new(),
            indicator_visible: false,
        }
    }

    /// Show the interactive-mode visual indicator.
    ///
    /// Adds a thin border and "Interactive" label to the scene.
    /// Returns `true` if the indicator was added (wasn't already visible).
    pub fn show_indicator(
        &mut self,
        scene: &mut crate::scene::Scene,
        width: f32,
        height: f32,
    ) -> bool {
        if self.indicator_visible {
            return false;
        }

        let border_color = crate::scene::Color::new(0.2, 0.8, 1.0, 0.6);
        let border_thickness = 3.0;

        // Top border
        let top = scene.add_rect(crate::scene::RectProps {
            x: 0.0,
            y: 0.0,
            width,
            height: border_thickness,
            color: border_color,
        });
        // Bottom border
        let bottom = scene.add_rect(crate::scene::RectProps {
            x: 0.0,
            y: height - border_thickness,
            width,
            height: border_thickness,
            color: border_color,
        });
        // Left border
        let left = scene.add_rect(crate::scene::RectProps {
            x: 0.0,
            y: border_thickness,
            width: border_thickness,
            height: height - 2.0 * border_thickness,
            color: border_color,
        });
        // Right border
        let right = scene.add_rect(crate::scene::RectProps {
            x: width - border_thickness,
            y: border_thickness,
            width: border_thickness,
            height: height - 2.0 * border_thickness,
            color: border_color,
        });

        // "INTERACTIVE" label (top-right corner)
        let label = scene.add_text(crate::scene::TextProps {
            x: width - 180.0,
            y: 8.0,
            text: "INTERACTIVE".to_string(),
            font_size: 14.0,
            color: crate::scene::Color::new(0.2, 0.8, 1.0, 0.9),
        });

        self.indicator_node_ids = vec![top, bottom, left, right, label];
        self.indicator_visible = true;
        debug!("Interactive indicator shown ({} nodes)", self.indicator_node_ids.len());
        true
    }

    /// Hide the interactive-mode visual indicator.
    ///
    /// Removes the border and label nodes from the scene.
    /// Returns `true` if the indicator was removed (was visible).
    pub fn hide_indicator(&mut self, scene: &mut crate::scene::Scene) -> bool {
        if !self.indicator_visible {
            return false;
        }

        for id in self.indicator_node_ids.drain(..) {
            scene.remove(id);
        }
        self.indicator_visible = false;
        debug!("Interactive indicator hidden");
        true
    }

    /// Whether the indicator is currently visible.
    pub fn indicator_visible(&self) -> bool {
        self.indicator_visible
    }
}

impl Default for InputManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── HitTester tests ─────────────────────────────────────────────────

    #[test]
    fn hit_test_empty() {
        let tester = HitTester::new();
        assert_eq!(tester.hit_test(50.0, 50.0), None);
        assert!(tester.is_empty());
    }

    #[test]
    fn hit_test_single_rect_inside() {
        let mut tester = HitTester::new();
        let id = tester.add_rect(10.0, 10.0, 100.0, 50.0, 0);

        assert_eq!(tester.hit_test(50.0, 30.0), Some(id));
        assert_eq!(tester.hit_test(10.0, 10.0), Some(id)); // top-left corner
        assert_eq!(tester.hit_test(109.9, 59.9), Some(id)); // just inside bottom-right
    }

    #[test]
    fn hit_test_single_rect_outside() {
        let mut tester = HitTester::new();
        tester.add_rect(10.0, 10.0, 100.0, 50.0, 0);

        assert_eq!(tester.hit_test(0.0, 0.0), None); // top-left of screen
        assert_eq!(tester.hit_test(9.9, 10.0), None); // just left
        assert_eq!(tester.hit_test(110.0, 30.0), None); // right edge (exclusive)
        assert_eq!(tester.hit_test(50.0, 60.0), None); // bottom edge (exclusive)
        assert_eq!(tester.hit_test(200.0, 200.0), None); // far away
    }

    #[test]
    fn hit_test_z_order_topmost_wins() {
        let mut tester = HitTester::new();
        // Two overlapping rects: bottom (z=0) and top (z=10)
        let id_bottom = tester.add_rect(0.0, 0.0, 200.0, 200.0, 0);
        let id_top = tester.add_rect(50.0, 50.0, 100.0, 100.0, 10);

        // In the overlap zone, top wins
        assert_eq!(tester.hit_test(75.0, 75.0), Some(id_top));

        // Outside top but inside bottom, bottom wins
        assert_eq!(tester.hit_test(10.0, 10.0), Some(id_bottom));
    }

    #[test]
    fn hit_test_same_z_order_first_added_checked_first() {
        let mut tester = HitTester::new();
        // Same z_order — stable sort means first added is tested first
        let id_first = tester.add_rect(0.0, 0.0, 100.0, 100.0, 0);
        let _id_second = tester.add_rect(50.0, 50.0, 100.0, 100.0, 0);

        // In overlap zone, first added (same z) wins due to stable sort
        assert_eq!(tester.hit_test(75.0, 75.0), Some(id_first));
    }

    #[test]
    fn hit_test_remove_rect() {
        let mut tester = HitTester::new();
        let id = tester.add_rect(10.0, 10.0, 100.0, 50.0, 0);

        assert_eq!(tester.hit_test(50.0, 30.0), Some(id));
        assert!(tester.remove_rect(id));
        assert_eq!(tester.hit_test(50.0, 30.0), None);
        assert!(!tester.remove_rect(id)); // already removed
    }

    #[test]
    fn hit_test_clear() {
        let mut tester = HitTester::new();
        tester.add_rect(0.0, 0.0, 10.0, 10.0, 0);
        tester.add_rect(20.0, 20.0, 10.0, 10.0, 0);
        assert_eq!(tester.len(), 2);

        tester.clear();
        assert!(tester.is_empty());
        assert_eq!(tester.hit_test(5.0, 5.0), None);
    }

    #[test]
    fn hit_test_negative_coords() {
        let mut tester = HitTester::new();
        let id = tester.add_rect(-50.0, -50.0, 100.0, 100.0, 0);

        assert_eq!(tester.hit_test(-25.0, -25.0), Some(id));
        assert_eq!(tester.hit_test(49.9, 49.9), Some(id));
        assert_eq!(tester.hit_test(50.0, 50.0), None);
    }

    #[test]
    fn hit_test_zero_size_rect() {
        let mut tester = HitTester::new();
        tester.add_rect(10.0, 10.0, 0.0, 0.0, 0);

        // Zero-size rect should not contain any point
        assert_eq!(tester.hit_test(10.0, 10.0), None);
    }

    #[test]
    fn hit_test_many_rects_z_order() {
        let mut tester = HitTester::new();
        // Stack of 5 rects at the same position with increasing z
        let mut ids = Vec::new();
        for z in 0..5 {
            let id = tester.add_rect(0.0, 0.0, 100.0, 100.0, z);
            ids.push(id);
        }

        // Highest z (4) should win
        assert_eq!(tester.hit_test(50.0, 50.0), Some(ids[4]));
    }

    // ── InputMode / OverlayInputState tests ─────────────────────────────

    #[test]
    fn state_starts_passive() {
        let state = OverlayInputState::new(4000);
        assert_eq!(state.mode, InputMode::Passive);
        assert!(!state.is_interactive());
        assert!(state.interactive_since.is_none());
    }

    #[test]
    fn state_enter_interactive() {
        let mut state = OverlayInputState::new(4000);

        let changed = state.enter_interactive();
        assert!(changed);
        assert_eq!(state.mode, InputMode::Interactive);
        assert!(state.is_interactive());
        assert!(state.interactive_since.is_some());
    }

    #[test]
    fn state_enter_interactive_twice_is_reset() {
        let mut state = OverlayInputState::new(4000);

        state.enter_interactive();
        let first_since = state.interactive_since.unwrap();

        // Small delay to distinguish timestamps
        std::thread::sleep(std::time::Duration::from_millis(5));

        let changed = state.enter_interactive();
        assert!(!changed); // mode didn't change (already interactive)
        assert!(state.interactive_since.unwrap() >= first_since);
    }

    #[test]
    fn state_round_trip() {
        let mut state = OverlayInputState::new(4000);

        state.enter_interactive();
        assert!(state.is_interactive());

        let changed = state.enter_passive();
        assert!(changed);
        assert!(!state.is_interactive());
        assert!(state.interactive_since.is_none());
    }

    #[test]
    fn state_enter_passive_when_already_passive() {
        let mut state = OverlayInputState::new(4000);

        let changed = state.enter_passive();
        assert!(!changed); // already passive
    }

    #[test]
    fn state_timeout_duration() {
        let state = OverlayInputState::new(3000);
        assert_eq!(state.timeout, Duration::from_millis(3000));
    }

    // ── InteractiveRect tests ───────────────────────────────────────────

    #[test]
    fn rect_contains_basic() {
        let rect = InteractiveRect {
            id: 0,
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
            z_order: 0,
        };

        assert!(rect.contains(10.0, 20.0)); // top-left
        assert!(rect.contains(109.9, 69.9)); // near bottom-right
        assert!(!rect.contains(110.0, 70.0)); // at bottom-right edge (exclusive)
        assert!(!rect.contains(9.9, 20.0)); // just outside left
    }
}
