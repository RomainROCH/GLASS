//! Clock module: displays local time in the overlay.
//!
//! Configurable time format (strftime). Updates every second by default.

use super::{remove_nodes, ModuleInfo, OverlayModule};
use crate::scene::{Color, NodeId, Scene, SceneNode, TextProps};
use std::time::{Duration, Instant};
use tracing::debug;

/// Default Y position for the clock (top-left area).
const DEFAULT_Y: f32 = 10.0;
/// Default X position.
const DEFAULT_X: f32 = 10.0;
/// Default font size.
const FONT_SIZE: f32 = 18.0;
/// Default update interval.
const UPDATE_INTERVAL: Duration = Duration::from_secs(1);

/// Clock overlay module.
#[derive(Debug)]
pub struct ClockModule {
    enabled: bool,
    format: String,
    node_id: Option<NodeId>,
    last_update: Instant,
    last_text: String,
    base_x: f32,
    base_y: f32,
}

impl ClockModule {
    /// Create a new clock module with the given time format.
    pub fn new(format: &str) -> Self {
        Self {
            enabled: true,
            format: format.to_string(),
            node_id: None,
            last_update: Instant::now(),
            last_text: String::new(),
            base_x: DEFAULT_X,
            base_y: DEFAULT_Y,
        }
    }

    /// Update the display format string.
    pub fn set_format(&mut self, format: &str) {
        self.format = format.to_string();
        // Force refresh on next update
        self.last_text.clear();
    }

    fn current_time_text(&self) -> String {
        let now = chrono::Local::now();
        now.format(&self.format).to_string()
    }
}

impl OverlayModule for ClockModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "clock",
            name: "Clock",
            description: "Displays local time (configurable format)",
        }
    }

    fn init(&mut self, scene: &mut Scene) {
        let text = self.current_time_text();
        self.last_text = text.clone();
        let id = scene.add_text(TextProps {
            x: self.base_x,
            y: self.base_y,
            text,
            font_size: FONT_SIZE,
            color: Color::new(1.0, 1.0, 1.0, 0.85),
        });
        self.node_id = Some(id);
        self.last_update = Instant::now();
        debug!("Clock module initialized");
    }

    fn update(&mut self, scene: &mut Scene, _dt: Duration) -> bool {
        if self.last_update.elapsed() < UPDATE_INTERVAL {
            return false;
        }
        self.last_update = Instant::now();

        let text = self.current_time_text();
        if text == self.last_text {
            return false;
        }
        self.last_text = text.clone();

        if let Some(id) = self.node_id {
            scene.update(
                id,
                SceneNode::Text(TextProps {
                    x: self.base_x,
                    y: self.base_y,
                    text,
                    font_size: FONT_SIZE,
                    color: Color::new(1.0, 1.0, 1.0, 0.85),
                }),
            );
            true
        } else {
            false
        }
    }

    fn deinit(&mut self, scene: &mut Scene) {
        let mut ids: Vec<NodeId> = self.node_id.take().into_iter().collect();
        remove_nodes(scene, &mut ids);
        self.last_text.clear();
        debug!("Clock module deinitialized");
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
        self.base_x = x;
        self.base_y = y;
    }

    fn content_size(&self) -> (f32, f32) {
        (150.0, FONT_SIZE + 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_produces_text() {
        let clock = ClockModule::new("%H:%M:%S");
        let text = clock.current_time_text();
        // Should be in HH:MM:SS format
        assert_eq!(text.len(), 8);
        assert_eq!(&text[2..3], ":");
        assert_eq!(&text[5..6], ":");
    }

    #[test]
    fn clock_init_adds_node() {
        let mut clock = ClockModule::new("%H:%M");
        let mut scene = Scene::new();
        clock.init(&mut scene);
        assert_eq!(scene.len(), 1);
        assert!(clock.node_id.is_some());
    }

    #[test]
    fn clock_deinit_removes_node() {
        let mut clock = ClockModule::new("%H:%M");
        let mut scene = Scene::new();
        clock.init(&mut scene);
        assert_eq!(scene.len(), 1);
        clock.deinit(&mut scene);
        assert_eq!(scene.len(), 0);
        assert!(clock.node_id.is_none());
    }

    #[test]
    fn clock_format_change() {
        let mut clock = ClockModule::new("%H:%M");
        clock.set_format("%Y-%m-%d");
        let text = clock.current_time_text();
        // Should be YYYY-MM-DD format (10 chars)
        assert_eq!(text.len(), 10);
    }

    // ── Edge cases ───────────────────────────────────────────────────────

    #[test]
    fn empty_format_string_does_not_panic() {
        let clock = ClockModule::new("");
        // chrono formats an empty string as an empty string — must not panic
        let text = clock.current_time_text();
        assert_eq!(
            text, "",
            "empty format should produce empty string, not panic"
        );
    }

    #[test]
    fn set_position_stores_base_coordinates() {
        let mut clock = ClockModule::new("%H:%M");
        clock.set_position(55.0, 120.0);
        assert_eq!(clock.base_x, 55.0, "base_x should reflect set_position x");
        assert_eq!(clock.base_y, 120.0, "base_y should reflect set_position y");
    }

    #[test]
    fn set_format_clears_last_text_to_force_refresh() {
        let mut clock = ClockModule::new("%H:%M");
        // Simulate a previous render that cached some text
        clock.last_text = "12:00".to_string();
        clock.set_format("%S");
        assert!(
            clock.last_text.is_empty(),
            "set_format should clear last_text to force a refresh on next update"
        );
    }

    #[test]
    fn content_size_is_nonzero() {
        let clock = ClockModule::new("%H:%M:%S");
        let (w, h) = clock.content_size();
        assert!(w > 0.0, "content width must be positive");
        assert!(h > 0.0, "content height must be positive");
    }
}
