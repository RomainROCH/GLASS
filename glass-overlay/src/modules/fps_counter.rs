//! Overlay-only FPS counter module.
//!
//! Measures the overlay's own rendering frame rate — NOT the game's FPS.
//! The label explicitly states "overlay-only FPS" to avoid misleading users.
//!
//! Uses a simple rolling-window averaging approach with no heap allocations
//! in steady state.

use super::{remove_nodes, ModuleInfo, OverlayModule};
use crate::scene::{Color, NodeId, Scene, SceneNode, TextProps};
use std::time::{Duration, Instant};
use tracing::debug;

/// Default Y position (below system stats).
const DEFAULT_Y: f32 = 60.0;
/// Default X position.
const DEFAULT_X: f32 = 10.0;
/// Font size.
const FONT_SIZE: f32 = 14.0;
/// Display refresh interval — update the shown FPS value this often.
const DISPLAY_INTERVAL: Duration = Duration::from_millis(500);

/// Overlay-only FPS counter module.
///
/// **Important**: This measures overlay render frames, not game FPS.
/// The displayed label always includes "overlay-only" provenance.
#[derive(Debug)]
pub struct FpsCounterModule {
    enabled: bool,
    node_id: Option<NodeId>,
    /// Frame timestamps for FPS calculation (ring buffer).
    frame_times: [Instant; 64],
    /// Write index into the ring buffer.
    write_idx: usize,
    /// Number of frames recorded.
    frame_count: u32,
    /// Last time the display was updated.
    last_display_update: Instant,
    /// Last displayed FPS value.
    last_fps_text: String,
    base_x: f32,
    base_y: f32,
}

impl FpsCounterModule {
    /// Create a new FPS counter module.
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            enabled: true,
            node_id: None,
            frame_times: [now; 64],
            write_idx: 0,
            frame_count: 0,
            last_display_update: now,
            last_fps_text: String::new(),
            base_x: DEFAULT_X,
            base_y: DEFAULT_Y,
        }
    }

    /// Record a frame render event. Call this after every `render()`.
    pub fn record_frame(&mut self) {
        self.frame_times[self.write_idx] = Instant::now();
        self.write_idx = (self.write_idx + 1) % self.frame_times.len();
        self.frame_count = self.frame_count.saturating_add(1);
    }

    /// Calculate the current FPS from the ring buffer.
    fn calculate_fps(&self) -> f32 {
        let buf_len = self.frame_times.len();
        let count = (self.frame_count as usize).min(buf_len);
        if count < 2 {
            return 0.0;
        }

        // Find the oldest and newest timestamps in the used portion
        let newest_idx = if self.write_idx == 0 {
            buf_len - 1
        } else {
            self.write_idx - 1
        };

        let oldest_idx = if self.frame_count as usize >= buf_len {
            self.write_idx // write_idx is where the oldest entry was overwritten
        } else {
            0
        };

        let newest = self.frame_times[newest_idx];
        let oldest = self.frame_times[oldest_idx];
        let elapsed = newest.duration_since(oldest);

        if elapsed.as_secs_f32() < 0.001 {
            return 0.0;
        }

        (count - 1) as f32 / elapsed.as_secs_f32()
    }
}

impl Default for FpsCounterModule {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlayModule for FpsCounterModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "fps",
            name: "Overlay FPS",
            description: "Estimated overlay-only FPS (does not reflect game FPS)",
        }
    }

    fn init(&mut self, scene: &mut Scene) {
        let text = "overlay-only FPS: --".to_string();
        self.last_fps_text = text.clone();
        let id = scene.add_text(TextProps {
            x: self.base_x,
            y: self.base_y,
            text,
            font_size: FONT_SIZE,
            color: Color::new(0.6, 1.0, 0.6, 0.75),
        });
        self.node_id = Some(id);
        self.last_display_update = Instant::now();
        debug!("FPS counter module initialized");
    }

    fn update(&mut self, scene: &mut Scene, _dt: Duration) -> bool {
        if self.last_display_update.elapsed() < DISPLAY_INTERVAL {
            return false;
        }
        self.last_display_update = Instant::now();

        let fps = self.calculate_fps();
        let text = if fps > 0.1 {
            format!("overlay-only FPS: {:.0}", fps)
        } else {
            "overlay-only FPS: --".to_string()
        };

        if text == self.last_fps_text {
            return false;
        }
        self.last_fps_text = text.clone();

        if let Some(id) = self.node_id {
            scene.update(
                id,
                SceneNode::Text(TextProps {
                    x: self.base_x,
                    y: self.base_y,
                    text,
                    font_size: FONT_SIZE,
                    color: Color::new(0.6, 1.0, 0.6, 0.75),
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
        self.frame_count = 0;
        self.last_fps_text.clear();
        debug!("FPS counter module deinitialized");
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
        (220.0, FONT_SIZE + 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_init_adds_node() {
        let mut module = FpsCounterModule::new();
        let mut scene = Scene::new();
        module.init(&mut scene);
        assert_eq!(scene.len(), 1);
        assert!(module.node_id.is_some());
    }

    #[test]
    fn fps_deinit_removes_node() {
        let mut module = FpsCounterModule::new();
        let mut scene = Scene::new();
        module.init(&mut scene);
        module.deinit(&mut scene);
        assert_eq!(scene.len(), 0);
    }

    #[test]
    fn fps_record_frames() {
        let mut module = FpsCounterModule::new();
        // Record many frames quickly
        for _ in 0..10 {
            module.record_frame();
        }
        // FPS should be calculable (though value depends on timing)
        let fps = module.calculate_fps();
        // Just verify it doesn't panic and returns something reasonable
        assert!(fps >= 0.0);
    }

    #[test]
    fn fps_label_shows_overlay_only() {
        let module = FpsCounterModule::new();
        let info = module.info();
        assert!(info.description.contains("overlay-only"));
        assert!(info.description.contains("does not reflect game FPS"));
    }

    #[test]
    fn fps_initial_text_says_overlay_only() {
        let mut module = FpsCounterModule::new();
        let mut scene = Scene::new();
        module.init(&mut scene);
        assert!(module.last_fps_text.contains("overlay-only FPS"));
    }
}
