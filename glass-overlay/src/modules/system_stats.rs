//! System stats module: CPU + memory usage with provenance labels.
//!
//! Uses the `sysinfo` crate for cross-platform metrics.
//! All labels include "system:" prefix to indicate provenance.
//! Gracefully degrades if metrics are unavailable.

use super::{ModuleInfo, OverlayModule, remove_nodes};
use crate::scene::{Color, NodeId, Scene, SceneNode, TextProps};
use std::time::{Duration, Instant};
use sysinfo::System;
use tracing::debug;

/// Default Y position (below clock).
const DEFAULT_Y: f32 = 34.0;
/// Default X position.
const DEFAULT_X: f32 = 10.0;
/// Font size.
const FONT_SIZE: f32 = 14.0;
/// Default refresh interval.
const DEFAULT_INTERVAL: Duration = Duration::from_secs(2);

/// System stats overlay module.
pub struct SystemStatsModule {
    enabled: bool,
    node_ids: Vec<NodeId>,
    sys: System,
    interval: Duration,
    last_update: Instant,
    last_cpu_text: String,
    last_mem_text: String,
    base_x: f32,
    base_y: f32,
}

impl SystemStatsModule {
    /// Create a new system stats module.
    pub fn new() -> Self {
        Self {
            enabled: true,
            node_ids: Vec::new(),
            sys: System::new(),
            interval: DEFAULT_INTERVAL,
            last_update: Instant::now() - DEFAULT_INTERVAL, // force immediate first update
            last_cpu_text: String::new(),
            last_mem_text: String::new(),
            base_x: DEFAULT_X,
            base_y: DEFAULT_Y,
        }
    }

    /// Set the refresh interval.
    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    fn refresh_metrics(&mut self) -> (String, String) {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();

        let cpu_usage = self.sys.global_cpu_usage();
        let cpu_text = format!("system: CPU {:.0}%", cpu_usage);

        let used_mem = self.sys.used_memory();
        let total_mem = self.sys.total_memory();
        let mem_text = if total_mem > 0 {
            let used_gib = used_mem as f64 / (1024.0 * 1024.0 * 1024.0);
            let total_gib = total_mem as f64 / (1024.0 * 1024.0 * 1024.0);
            format!("system: RAM {:.1}/{:.1} GiB", used_gib, total_gib)
        } else {
            "system: RAM N/A".to_string()
        };

        (cpu_text, mem_text)
    }
}

impl Default for SystemStatsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlayModule for SystemStatsModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "system_stats",
            name: "System Stats",
            description: "CPU and memory usage (system-reported)",
        }
    }

    fn init(&mut self, scene: &mut Scene) {
        let (cpu_text, mem_text) = self.refresh_metrics();
        self.last_cpu_text = cpu_text.clone();
        self.last_mem_text = mem_text.clone();

        let color = Color::new(0.8, 0.9, 1.0, 0.75);

        let cpu_id = scene.add_text(TextProps {
            x: self.base_x,
            y: self.base_y,
            text: cpu_text,
            font_size: FONT_SIZE,
            color,
        });
        let mem_id = scene.add_text(TextProps {
            x: self.base_x,
            y: self.base_y + FONT_SIZE * 1.3,
            text: mem_text,
            font_size: FONT_SIZE,
            color,
        });

        self.node_ids = vec![cpu_id, mem_id];
        self.last_update = Instant::now();
        debug!("System stats module initialized");
    }

    fn update(&mut self, scene: &mut Scene, _dt: Duration) -> bool {
        if self.last_update.elapsed() < self.interval {
            return false;
        }
        self.last_update = Instant::now();

        let (cpu_text, mem_text) = self.refresh_metrics();

        let cpu_changed = cpu_text != self.last_cpu_text;
        let mem_changed = mem_text != self.last_mem_text;

        if !cpu_changed && !mem_changed {
            return false;
        }

        let color = Color::new(0.8, 0.9, 1.0, 0.75);

        if cpu_changed {
            self.last_cpu_text = cpu_text.clone();
            if let Some(&id) = self.node_ids.first() {
                scene.update(
                    id,
                    SceneNode::Text(TextProps {
                        x: self.base_x,
                        y: self.base_y,
                        text: cpu_text,
                        font_size: FONT_SIZE,
                        color,
                    }),
                );
            }
        }

        if mem_changed {
            self.last_mem_text = mem_text.clone();
            if let Some(&id) = self.node_ids.get(1) {
                scene.update(
                    id,
                    SceneNode::Text(TextProps {
                        x: self.base_x,
                        y: self.base_y + FONT_SIZE * 1.3,
                        text: mem_text,
                        font_size: FONT_SIZE,
                        color,
                    }),
                );
            }
        }

        true
    }

    fn deinit(&mut self, scene: &mut Scene) {
        remove_nodes(scene, &mut self.node_ids);
        self.last_cpu_text.clear();
        self.last_mem_text.clear();
        debug!("System stats module deinitialized");
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
        (250.0, FONT_SIZE * 2.6 + 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_produces_labeled_text() {
        let mut module = SystemStatsModule::new();
        let (cpu, mem) = module.refresh_metrics();
        assert!(cpu.starts_with("system: CPU "));
        assert!(mem.starts_with("system: RAM "));
    }

    #[test]
    fn stats_init_adds_two_nodes() {
        let mut module = SystemStatsModule::new();
        let mut scene = Scene::new();
        module.init(&mut scene);
        assert_eq!(scene.len(), 2);
        assert_eq!(module.node_ids.len(), 2);
    }

    #[test]
    fn stats_deinit_cleans_up() {
        let mut module = SystemStatsModule::new();
        let mut scene = Scene::new();
        module.init(&mut scene);
        module.deinit(&mut scene);
        assert_eq!(scene.len(), 0);
        assert!(module.node_ids.is_empty());
    }
}
