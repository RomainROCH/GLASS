//! Overlay module system: composable HUD modules with runtime toggle and config.
//!
//! Most applications should use built-in modules through
//! [`crate::LayoutManager`] + [`crate::WidgetWrapper`], which handle positioning
//! and lifecycle together.
//!
//! [`ModuleRegistry`] is the lower-level/manual path for applications that want
//! to manage a flat set of modules directly without the layout system.
//!
//! Modules produce scene-graph nodes that the renderer draws. Module state
//! (enabled/disabled + per-module config) is persisted via the hot-reload
//! config system.
//!
//! # Built-in Modules
//! - [`clock`] — Local clock with configurable format
//! - [`system_stats`] — CPU + memory usage (system-reported)
//! - [`fps_counter`] — Overlay-only FPS estimator

pub mod clock;
pub mod fps_counter;
pub mod system_stats;

pub use clock::ClockModule;
pub use fps_counter::FpsCounterModule;
pub use system_stats::SystemStatsModule;

use crate::scene::{NodeId, Scene};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// ─── OverlayModule trait ────────────────────────────────────────────────────

/// Metadata returned by every module.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    /// Unique machine-readable identifier (e.g. `"clock"`).
    pub id: &'static str,
    /// Human-readable display name.
    pub name: &'static str,
    /// Short description of what the module shows.
    pub description: &'static str,
}

/// Trait implemented by all overlay modules.
///
/// Lifecycle:
/// 1. `info()` — return module metadata (const)
/// 2. `init(scene)` — add initial scene nodes
/// 3. `update(scene, dt)` — called every tick; update content if needed
/// 4. `deinit(scene)` — remove scene nodes on disable/shutdown
pub trait OverlayModule {
    /// Return module metadata.
    fn info(&self) -> ModuleInfo;

    /// Initialize module: add nodes to the scene graph.
    fn init(&mut self, scene: &mut Scene);

    /// Periodic update: refresh module content.
    ///
    /// `dt` is the time elapsed since the last update call.
    /// Returns `true` if the scene was modified (needs re-render).
    fn update(&mut self, scene: &mut Scene, dt: Duration) -> bool;

    /// Clean up: remove all nodes added by this module.
    fn deinit(&mut self, scene: &mut Scene);

    /// Whether the module is currently enabled.
    fn enabled(&self) -> bool;

    /// Enable or disable the module.
    fn set_enabled(&mut self, enabled: bool);

    /// Downcast support for per-module config updates.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// Set the base rendering position for this module.
    ///
    /// Called by the layout system when the widget's position is computed.
    /// Modules should store `(x, y)` and use them when creating/updating
    /// scene nodes.
    fn set_position(&mut self, _x: f32, _y: f32) {}

    /// Return the estimated content size `(width, height)` in pixels.
    ///
    /// Used by the layout system for anchor resolution and hit-testing.
    /// Implementations should return a conservative estimate covering all
    /// scene nodes created by the module.
    fn content_size(&self) -> (f32, f32) {
        (0.0, 0.0)
    }
}

// ─── ModuleRegistry ─────────────────────────────────────────────────────────

/// Configuration for all modules (serialized in the config file).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModulesConfig {
    /// Enable the clock module.
    #[serde(default = "default_true")]
    pub clock_enabled: bool,
    /// Clock display format (strftime-compatible).
    #[serde(default = "default_clock_format")]
    pub clock_format: String,
    /// Enable the system stats module.
    #[serde(default = "default_true")]
    pub system_stats_enabled: bool,
    /// System stats refresh interval in milliseconds.
    #[serde(default = "default_stats_interval_ms")]
    pub stats_interval_ms: u64,
    /// Enable the overlay-only FPS counter.
    #[serde(default = "default_true")]
    pub fps_enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_clock_format() -> String {
    "%H:%M:%S".to_string()
}

fn default_stats_interval_ms() -> u64 {
    2000
}

impl Default for ModulesConfig {
    fn default() -> Self {
        Self {
            clock_enabled: true,
            clock_format: default_clock_format(),
            system_stats_enabled: true,
            stats_interval_ms: default_stats_interval_ms(),
            fps_enabled: true,
        }
    }
}

/// Registry that owns and manages all overlay modules.
///
/// Thread safety: single-threaded — used exclusively from the message-loop thread.
pub struct ModuleRegistry {
    modules: Vec<Box<dyn OverlayModule>>,
}

impl ModuleRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
        }
    }

    /// Register a module. Panics if a module with the same ID already exists.
    pub fn register(&mut self, module: Box<dyn OverlayModule>) {
        let id = module.info().id;
        assert!(
            !self.modules.iter().any(|m| m.info().id == id),
            "Duplicate module ID: {id}"
        );
        self.modules.push(module);
    }

    /// Initialize all enabled modules (add scene nodes).
    pub fn init_all(&mut self, scene: &mut Scene) {
        for module in &mut self.modules {
            if module.enabled() {
                module.init(scene);
            }
        }
    }

    /// Update all enabled modules. Returns `true` if any module modified the scene.
    pub fn update_all(&mut self, scene: &mut Scene, dt: Duration) -> bool {
        let mut dirty = false;
        for module in &mut self.modules {
            if module.enabled() {
                dirty |= module.update(scene, dt);
            }
        }
        dirty
    }

    /// Deinitialize all modules (remove scene nodes).
    pub fn deinit_all(&mut self, scene: &mut Scene) {
        for module in &mut self.modules {
            module.deinit(scene);
        }
    }

    /// Enable or disable a module by ID.
    ///
    /// If enabling, calls `init`. If disabling, calls `deinit`.
    /// Returns `true` if the module was found.
    pub fn set_enabled(&mut self, id: &str, enabled: bool, scene: &mut Scene) -> bool {
        if let Some(module) = self.modules.iter_mut().find(|m| m.info().id == id) {
            let was_enabled = module.enabled();
            module.set_enabled(enabled);
            if enabled && !was_enabled {
                module.init(scene);
            } else if !enabled && was_enabled {
                module.deinit(scene);
            }
            true
        } else {
            false
        }
    }

    /// Get the number of registered modules.
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// List all module infos and their enabled state.
    pub fn list(&self) -> Vec<(ModuleInfo, bool)> {
        self.modules
            .iter()
            .map(|m| (m.info(), m.enabled()))
            .collect()
    }

    /// Apply config to modules (enable/disable + per-module settings).
    pub fn apply_config(&mut self, config: &ModulesConfig, scene: &mut Scene) {
        self.set_enabled("clock", config.clock_enabled, scene);
        self.set_enabled("system_stats", config.system_stats_enabled, scene);
        self.set_enabled("fps", config.fps_enabled, scene);

        // Update clock format
        for module in &mut self.modules {
            if module.info().id == "clock" {
                if let Some(clock) = module.as_any_mut().downcast_mut::<clock::ClockModule>()
                {
                    clock.set_format(&config.clock_format);
                }
            }
            if module.info().id == "system_stats" {
                if let Some(stats) = module.as_any_mut().downcast_mut::<system_stats::SystemStatsModule>()
                {
                    stats.set_interval(Duration::from_millis(config.stats_interval_ms));
                }
            }
        }
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: remove a list of node IDs from the scene.
pub(crate) fn remove_nodes(scene: &mut Scene, ids: &mut Vec<NodeId>) {
    for id in ids.drain(..) {
        scene.remove(id);
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;

    /// Minimal test module for registry tests.
    struct DummyModule {
        enabled: bool,
        init_called: bool,
        deinit_called: bool,
        node_ids: Vec<NodeId>,
    }

    impl DummyModule {
        fn new(enabled: bool) -> Self {
            Self {
                enabled,
                init_called: false,
                deinit_called: false,
                node_ids: Vec::new(),
            }
        }
    }

    impl OverlayModule for DummyModule {
        fn info(&self) -> ModuleInfo {
            ModuleInfo {
                id: "dummy",
                name: "Dummy Module",
                description: "Test module",
            }
        }

        fn init(&mut self, scene: &mut Scene) {
            self.init_called = true;
            let id = scene.add_text(crate::scene::TextProps {
                x: 0.0,
                y: 0.0,
                text: "dummy".into(),
                font_size: 12.0,
                color: crate::scene::Color::WHITE,
            });
            self.node_ids.push(id);
        }

        fn update(&mut self, _scene: &mut Scene, _dt: Duration) -> bool {
            false
        }

        fn deinit(&mut self, scene: &mut Scene) {
            self.deinit_called = true;
            remove_nodes(scene, &mut self.node_ids);
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
    }

    #[test]
    fn register_and_list() {
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(DummyModule::new(true)));
        assert_eq!(registry.len(), 1);
        let list = registry.list();
        assert_eq!(list[0].0.id, "dummy");
        assert!(list[0].1); // enabled
    }

    #[test]
    #[should_panic(expected = "Duplicate module ID")]
    fn register_duplicate_panics() {
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(DummyModule::new(true)));
        registry.register(Box::new(DummyModule::new(false)));
    }

    #[test]
    fn init_all_only_enabled() {
        let mut registry = ModuleRegistry::new();
        let mut scene = Scene::new();

        // Register two different dummy modules (need different IDs)
        struct Dummy2(bool, Vec<NodeId>);
        impl OverlayModule for Dummy2 {
            fn info(&self) -> ModuleInfo {
                ModuleInfo {
                    id: "dummy2",
                    name: "Dummy2",
                    description: "",
                }
            }
            fn init(&mut self, scene: &mut Scene) {
                let id = scene.add_text(crate::scene::TextProps {
                    x: 0.0,
                    y: 0.0,
                    text: "d2".into(),
                    font_size: 12.0,
                    color: crate::scene::Color::WHITE,
                });
                self.1.push(id);
            }
            fn update(&mut self, _: &mut Scene, _: Duration) -> bool {
                false
            }
            fn deinit(&mut self, scene: &mut Scene) {
                remove_nodes(scene, &mut self.1);
            }
            fn enabled(&self) -> bool {
                self.0
            }
            fn set_enabled(&mut self, e: bool) {
                self.0 = e;
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        registry.register(Box::new(DummyModule::new(true)));
        registry.register(Box::new(Dummy2(false, Vec::new())));
        registry.init_all(&mut scene);

        // Only the enabled module should add nodes
        assert_eq!(scene.len(), 1);
    }

    #[test]
    fn enable_disable_lifecycle() {
        let mut registry = ModuleRegistry::new();
        let mut scene = Scene::new();

        registry.register(Box::new(DummyModule::new(false)));
        assert_eq!(scene.len(), 0);

        // Enable — should call init
        registry.set_enabled("dummy", true, &mut scene);
        assert_eq!(scene.len(), 1);

        // Disable — should call deinit
        registry.set_enabled("dummy", false, &mut scene);
        assert_eq!(scene.len(), 0);
    }

    #[test]
    fn set_enabled_unknown_module() {
        let mut registry = ModuleRegistry::new();
        let mut scene = Scene::new();
        assert!(!registry.set_enabled("nonexistent", true, &mut scene));
    }

    #[test]
    fn modules_config_roundtrip() {
        let config = ModulesConfig::default();
        let ron_str = ron::to_string(&config).unwrap();
        let parsed: ModulesConfig = ron::from_str(&ron_str).unwrap();
        assert_eq!(config, parsed);
    }
}
