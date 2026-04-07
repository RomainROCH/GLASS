//! System stats module: CPU usage, memory usage, and optional external temperature.
//!
//! Uses the `sysinfo` crate for cross-platform CPU and RAM metrics.
//! All labels carry a `"system:"` provenance prefix.
//! Temperature is deliberately NOT sourced internally — callers inject a
//! `TempSourceFn` callback via [`SystemStatsModule::set_temp_source`].
//! When no source is injected, or the source returns `None`, the CPU line
//! displays `"temp: N/A"` instead of a celsius value.
//! Gracefully degrades if any metric is unavailable.

use super::{remove_nodes, ModuleInfo, OverlayModule};
use crate::scene::{Color, NodeId, Scene, SceneNode, TextProps};
use std::fmt;
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

/// Optional external CPU temperature source.
///
/// The callback is invoked on every metrics refresh. Return `Some(celsius)`
/// when a reading is available, or `None` to signal unavailability.
/// The boxed closure is `Send` so the module can be moved across threads.
type TempSourceFn = Box<dyn FnMut() -> Option<f32> + Send>;

/// System stats overlay module.
///
/// Displays live CPU usage and RAM consumption as two overlay text nodes.
/// CPU text format:
/// - With temperature: `"system: CPU <pct>% · temp <celsius>°C"`
/// - Without temperature: `"system: CPU <pct>% · temp: N/A"`
///
/// Inject a temperature provider with [`Self::set_temp_source`] after
/// construction. GLASS itself has zero knowledge of hardware sensors.
pub struct SystemStatsModule {
    enabled: bool,
    node_ids: Vec<NodeId>,
    sys: System,
    /// Optional external CPU temperature callback.
    temp_source: Option<TempSourceFn>,
    interval: Duration,
    last_update: Instant,
    last_cpu_text: String,
    last_mem_text: String,
    base_x: f32,
    base_y: f32,
}

impl fmt::Debug for SystemStatsModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemStatsModule")
            .field("enabled", &self.enabled)
            .field("node_ids", &self.node_ids)
            .field("has_temp_source", &self.temp_source.is_some())
            .field("interval", &self.interval)
            .field("last_update", &self.last_update)
            .field("last_cpu_text", &self.last_cpu_text)
            .field("last_mem_text", &self.last_mem_text)
            .field("base_x", &self.base_x)
            .field("base_y", &self.base_y)
            .finish()
    }
}

impl SystemStatsModule {
    /// Create a new system stats module with no temperature source.
    ///
    /// Temperature will show as `"temp: N/A"` until a source is injected
    /// via [`Self::set_temp_source`].
    pub fn new() -> Self {
        Self {
            enabled: true,
            node_ids: Vec::new(),
            sys: System::new(),
            temp_source: None,
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

    /// Inject an external CPU temperature source.
    ///
    /// The callback is called on each metrics refresh. Return `Some(celsius)`
    /// when a reading is available, `None` otherwise. Passing a new source
    /// replaces any previously set one.
    pub fn set_temp_source(&mut self, source: TempSourceFn) {
        self.temp_source = Some(source);
    }

    fn refresh_metrics(&mut self) -> (String, String) {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();

        let cpu_usage = self.sys.global_cpu_usage();
        let temp_text = match &mut self.temp_source {
            Some(f) => match f() {
                Some(c) => format!("temp {:.0}°C", c),
                None => "temp: N/A".to_string(),
            },
            None => "temp: N/A".to_string(),
        };
        let cpu_text = format!("system: CPU {:.0}% · {temp_text}", cpu_usage);

        let used_mem = self.sys.used_memory();
        let total_mem = self.sys.total_memory();
        let mem_text = format_memory_text(used_mem, total_mem);

        (cpu_text, mem_text)
    }
}

impl Default for SystemStatsModule {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a memory usage string from raw byte counts.
///
/// Returns `"system: RAM <used>/<total> GiB"` when total memory is non-zero,
/// or `"system: RAM N/A"` when the system cannot report memory.
fn format_memory_text(used_bytes: u64, total_bytes: u64) -> String {
    if total_bytes > 0 {
        let used_gib = used_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let total_gib = total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        format!("system: RAM {:.1}/{:.1} GiB", used_gib, total_gib)
    } else {
        "system: RAM N/A".to_string()
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
        // No temp source injected — must show "temp: N/A"
        assert!(cpu.starts_with("system: CPU "), "cpu text: {cpu}");
        assert!(
            cpu.contains("temp: N/A"),
            "expected 'temp: N/A' in cpu text: {cpu}"
        );
        assert!(mem.starts_with("system: RAM "), "mem text: {mem}");
    }

    #[test]
    fn stats_with_temp_source_shows_celsius() {
        let mut module = SystemStatsModule::new();
        module.set_temp_source(Box::new(|| Some(65.0)));
        let (cpu, _mem) = module.refresh_metrics();
        assert!(
            cpu.contains("temp 65°C"),
            "expected 'temp 65°C' in cpu text: {cpu}"
        );
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

    // ── temp_source edge cases ───────────────────────────────────────────

    #[test]
    fn temp_source_returning_none_shows_na() {
        let mut module = SystemStatsModule::new();
        module.set_temp_source(Box::new(|| None));
        let (cpu, _) = module.refresh_metrics();
        assert!(
            cpu.contains("temp: N/A"),
            "temp source returning None should show 'temp: N/A': {cpu}"
        );
    }

    // ── memory formatting edge cases ─────────────────────────────────────

    #[test]
    fn memory_formatting_zero_total_shows_na() {
        let text = format_memory_text(0, 0);
        assert_eq!(
            text, "system: RAM N/A",
            "zero total memory should produce 'system: RAM N/A'"
        );
    }

    #[test]
    fn memory_formatting_exact_gib_values() {
        // 8 GiB used, 16 GiB total
        let used = 8u64 * 1024 * 1024 * 1024;
        let total = 16u64 * 1024 * 1024 * 1024;
        let text = format_memory_text(used, total);
        assert!(
            text.contains("8.0/16.0 GiB"),
            "exact GiB values should format cleanly: {text}"
        );
        assert!(
            text.starts_with("system: RAM "),
            "must carry 'system: RAM' prefix: {text}"
        );
    }

    #[test]
    fn memory_formatting_zero_used_nonzero_total() {
        let total = 4u64 * 1024 * 1024 * 1024;
        let text = format_memory_text(0, total);
        assert!(
            text.contains("0.0/4.0 GiB"),
            "zero used with nonzero total should format: {text}"
        );
    }

    // ── enabled / set_enabled ────────────────────────────────────────────

    #[test]
    fn set_enabled_false_makes_enabled_return_false() {
        let mut module = SystemStatsModule::new();
        assert!(module.enabled(), "new module should be enabled by default");
        module.set_enabled(false);
        assert!(
            !module.enabled(),
            "set_enabled(false) should disable the module"
        );
    }

    #[test]
    fn set_enabled_true_reenables_module() {
        let mut module = SystemStatsModule::new();
        module.set_enabled(false);
        module.set_enabled(true);
        assert!(
            module.enabled(),
            "set_enabled(true) should re-enable the module"
        );
    }

    // ── set_position ──────────────────────────────────────────────────────

    #[test]
    fn set_position_stores_base_coordinates() {
        let mut module = SystemStatsModule::new();
        module.set_position(42.0, 99.0);
        assert_eq!(module.base_x, 42.0, "base_x must reflect set_position x");
        assert_eq!(module.base_y, 99.0, "base_y must reflect set_position y");
    }

    // ── set_interval ─────────────────────────────────────────────────────

    #[test]
    fn set_interval_is_stored() {
        let mut module = SystemStatsModule::new();
        let new_interval = Duration::from_millis(500);
        module.set_interval(new_interval);
        assert_eq!(
            module.interval, new_interval,
            "set_interval should update the stored interval"
        );
    }

    // ── content_size ─────────────────────────────────────────────────────

    #[test]
    fn content_size_is_nonzero() {
        let module = SystemStatsModule::new();
        let (w, h) = module.content_size();
        assert!(w > 0.0, "content width must be positive");
        assert!(h > 0.0, "content height must be positive");
    }
}
