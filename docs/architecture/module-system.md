# Module & Layout System

← [Architecture Overview](ARCHITECTURE.md)

The module system is the primary extension point for GLASS. Every overlay widget
— built-in or custom — implements the `OverlayModule` trait, owns its scene
nodes, and participates in a well-defined lifecycle. The layout system positions
modules on screen using anchor-based placement.

**Source files:** `modules/mod.rs`, `modules/clock.rs`, `modules/system_stats.rs`,
`modules/fps_counter.rs`, `layout.rs`

---

## Table of Contents

- [OverlayModule Trait](#overlaymodule-trait)
- [ModuleInfo](#moduleinfo)
- [Module Lifecycle](#module-lifecycle)
- [ModuleRegistry — Low-Level Path](#moduleregistry--low-level-path)
- [LayoutManager + WidgetWrapper — High-Level Path](#layoutmanager--widgetwrapper--high-level-path)
- [Anchor System](#anchor-system)
- [ModulesConfig — Serialized Configuration](#modulesconfig--serialized-configuration)
- [Built-in Modules](#built-in-modules)
- [Data Injection Pattern](#data-injection-pattern)
- [Extension Points for Custom Modules](#extension-points-for-custom-modules)

---

## OverlayModule Trait

Defined in `modules/mod.rs`. This is the core extension point — any overlay
widget implements this trait.

```rust
pub trait OverlayModule {
    /// Return module metadata (id, name, description).
    fn info(&self) -> ModuleInfo;

    /// Initialize module: add nodes to the scene graph.
    fn init(&mut self, scene: &mut Scene);

    /// Periodic update: refresh module content.
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
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Set the base rendering position for this module.
    /// Called by the layout system when the widget's position is computed.
    /// Default implementation is a no-op.
    fn set_position(&mut self, _x: f32, _y: f32) {}

    /// Return the estimated content size (width, height) in pixels.
    /// Used by the layout system for anchor resolution and hit-testing.
    /// Default returns (0.0, 0.0).
    fn content_size(&self) -> (f32, f32) { (0.0, 0.0) }
}
```

### Key Design Points

- **Scene node ownership**: Each module creates and destroys its own scene nodes.
  Node IDs are stored internally (e.g. `Vec<NodeId>`) and cleaned up in `deinit`.
- **Dirty tracking**: `update` returns `true` only when the scene was actually
  modified, enabling the renderer to skip no-op frames.
- **Downcast via `as_any_mut`**: Enables type-specific configuration (e.g.
  `downcast_mut::<ClockModule>()` to call `set_format()`). This avoids trait
  method explosion for per-module settings.
- **Position delegation**: `set_position` is called by the layout system;
  modules store the coordinates and use them when creating/updating scene nodes.

---

## ModuleInfo

```rust
pub struct ModuleInfo {
    /// Unique machine-readable identifier: "clock", "system_stats", "fps"
    pub id: &'static str,
    /// Human-readable display name: "Clock", "System Stats", "Overlay FPS"
    pub name: &'static str,
    /// Short description of what the module shows.
    pub description: &'static str,
}
```

The `id` field is the primary key. It must be unique within a registry or layout
manager — registration panics on duplicate IDs.

---

## Module Lifecycle

```
info() → init(scene) → update(scene, dt)* → deinit(scene)
                ↑                                   │
                └───── re-enable ───────────────────┘
```

| Phase | What happens | Who calls it |
|---|---|---|
| `info()` | Returns metadata. Pure, no side effects. | Registry/layout on registration |
| `init(scene)` | Creates scene nodes (`add_text`, `add_rect`), stores `NodeId`s | Registry/layout when module is enabled |
| `update(scene, dt)` | Refreshes content based on elapsed time. Returns `true` if scene modified. | Registry/layout on each tick (only for enabled modules) |
| `deinit(scene)` | Removes all owned scene nodes. Use the `remove_nodes()` helper. | Registry/layout on disable or shutdown |
| `set_position(x, y)` | Called by layout when position is computed. Modules store coordinates. | `WidgetWrapper::recalculate` / `LayoutManager::recalculate` |
| `content_size()` | Returns `(width, height)` estimate for anchor resolution and hit-testing. | Layout system during position computation |

**Enable/disable transitions** are lifecycle events:
- Enabling an inactive module calls `init(scene)`.
- Disabling an active module calls `deinit(scene)`.
- This ensures scene nodes are always consistent with enabled state.

**Resize handling**: On `WM_SIZE` / `WM_DISPLAYCHANGE`, the layout manager
calls `recalculate`, which deinits and reinits modules whose position changed.

---

## ModuleRegistry — Low-Level Path

`ModuleRegistry` is a flat `Vec<Box<dyn OverlayModule>>` that manages module
lifecycle without layout awareness. Use this when you need direct control over
positioning or don't need anchor-based layout.

```rust
pub struct ModuleRegistry {
    modules: Vec<Box<dyn OverlayModule>>,
}
```

| Method | Description |
|---|---|
| `register(module)` | Adds a module. **Panics on duplicate ID.** |
| `init_all(scene)` | Initializes all **enabled** modules. |
| `update_all(scene, dt) → bool` | Updates all enabled modules, returns `true` if any modified the scene. |
| `deinit_all(scene)` | Cleans up all modules (enabled or not). |
| `set_enabled(id, enabled, scene) → bool` | Toggles a module with proper init/deinit lifecycle. Returns `true` if found. |
| `apply_config(config, scene)` | Applies `ModulesConfig` — enables/disables modules and pushes per-module settings (clock format, stats interval). |
| `len()` / `is_empty()` | Count queries. |
| `list() → Vec<(ModuleInfo, bool)>` | Lists all modules with their enabled state. |

**Threading model**: Single-threaded. Used exclusively from the message-loop
thread. No interior mutability, no locking.

---

## LayoutManager + WidgetWrapper — High-Level Path

**Most applications should use `LayoutManager` instead of `ModuleRegistry`
directly.** It provides the same lifecycle API plus anchor-based positioning
and resize resilience.

**Source:** `layout.rs`

### WidgetWrapper

Composition wrapper that positions an `OverlayModule` via an `Anchor`. The
wrapper manages **position and size**; the inner module manages **content**.

```rust
pub struct WidgetWrapper<M: OverlayModule> {
    module: M,
    anchor: Anchor,
    margin: (f32, f32),      // (margin_x, margin_y)
    bbox: BoundingBox,
    screen_size: (f32, f32),
}
```

| Method | Description |
|---|---|
| `new(module, anchor, margin_x, margin_y)` | Create a positioned wrapper. |
| `recalculate(screen_w, screen_h)` | Recompute position from anchor; calls `set_position` on the inner module. |
| `module()` / `module_mut()` | Access the inner module. |
| `set_anchor(anchor)` | Change anchor and recalculate if screen size is known. |

`WidgetWrapper` implements the `Widget` trait (`bounding_box`, `contains_point`,
`draw`), bridging the layout system and the module system.

### LayoutManager

Flat list of type-erased `LayoutEntry` structs, each holding a
`Box<dyn OverlayModule>` with its anchor and margin.

```rust
pub struct LayoutManager {
    entries: Vec<LayoutEntry>,
    screen_w: f32,
    screen_h: f32,
}
```

| Method | Description |
|---|---|
| `new(screen_w, screen_h)` | Create with initial screen dimensions. |
| `add_widget(wrapper)` | Register a positioned module. Computes initial position. **Panics on duplicate ID.** |
| `init_all(scene)` | Initialize all enabled modules. |
| `update_all(scene, dt) → bool` | Update all enabled modules. Returns dirty flag. |
| `deinit_all(scene)` | Clean up all modules. |
| `set_enabled(id, enabled, scene) → bool` | Toggle with init/deinit lifecycle. |
| `apply_config(config, scene)` | Apply `ModulesConfig` (enable/disable + per-module settings). |
| `recalculate(screen_w, screen_h, scene)` | Recompute all positions on resize. Deinit + reinit modules whose position changed. |
| `hit_test(x, y) → Option<&str>` | O(n) linear scan of enabled widgets. Returns module ID of first hit. |
| `list() → Vec<(ModuleInfo, bool)>` | List all modules with enabled state. |

**Hit-testing performance**: Linear scan over the flat widget list. With < 10
widgets (typical overlay), this is effectively O(1). No tree traversal, no
spatial indexing overhead.

**Resize resilience**: On `WM_SIZE` / `WM_DISPLAYCHANGE`, call `recalculate()`.
Modules whose position changed are deinit + reinit to recreate scene nodes at
the correct location. Only triggers on actual bounding box change.

---

## Anchor System

Defined in `layout.rs`. Anchors determine how a widget is positioned relative
to screen edges.

```rust
pub enum Anchor {
    TopLeft,                     // Default. Margin pushes right and down.
    TopRight,                    // Margin pushes left and down.
    BottomLeft,                  // Margin pushes right and up.
    BottomRight,                 // Margin pushes left and up.
    Center,                      // Margin shifts from center.
    ScreenPercentage(f32, f32),  // 0.0–1.0 of screen dimensions.
}
```

### Resolution

`Anchor::resolve(content_w, content_h, screen_w, screen_h, margin_x, margin_y) → (x, y)`

Computes the absolute top-left coordinate for a widget given its content size,
screen dimensions, and margin offset.

| Anchor | Resulting `(x, y)` |
|---|---|
| `TopLeft` | `(margin_x, margin_y)` |
| `TopRight` | `(screen_w - content_w - margin_x, margin_y)` |
| `BottomLeft` | `(margin_x, screen_h - content_h - margin_y)` |
| `BottomRight` | `(screen_w - content_w - margin_x, screen_h - content_h - margin_y)` |
| `Center` | `((screen_w - content_w) / 2 + margin_x, (screen_h - content_h) / 2 + margin_y)` |
| `ScreenPercentage(px, py)` | `(screen_w * px + margin_x, screen_h * py + margin_y)` |

### WidgetLayoutConfig (serialized)

Per-widget layout settings in the config file:

```rust
pub struct WidgetLayoutConfig {
    pub anchor: Anchor,    // Default: TopLeft
    pub margin_x: f32,     // Default: 10.0
    pub margin_y: f32,     // Default: 10.0
}
```

---

## ModulesConfig — Serialized Configuration

Persisted in the RON/TOML config file under the `modules` section. Applied via
`apply_config()` on either `ModuleRegistry` or `LayoutManager`.

```rust
pub struct ModulesConfig {
    pub clock_enabled: bool,        // Default: true
    pub clock_format: String,       // Default: "%H:%M:%S"
    pub system_stats_enabled: bool, // Default: true
    pub stats_interval_ms: u64,     // Default: 2000
    pub fps_enabled: bool,          // Default: true
}
```

`apply_config` performs:
1. Enable/disable each built-in module (`set_enabled` with lifecycle).
2. Push per-module settings via downcast:
   - `ClockModule::set_format(clock_format)` — updates the strftime format.
   - `SystemStatsModule::set_interval(stats_interval_ms)` — changes the refresh
     interval.

This integrates with the [config hot-reload system](config-system.md) — when
the config file changes, the new `ModulesConfig` is applied to the running
layout.

---

## Built-in Modules

### ClockModule (`modules/clock.rs`)

Displays the local time in a configurable strftime format.

| Property | Value |
|---|---|
| ID | `"clock"` |
| Name | `"Clock"` |
| Scene nodes | 1 text node |
| Update interval | 1 second (internal, checks elapsed time) |
| Content size | `(150.0, 22.0)` |
| Configurable | Format string via `set_format()` or `ModulesConfig::clock_format` |

### SystemStatsModule (`modules/system_stats.rs`)

Displays CPU usage, memory usage, and optional temperature. Uses the `sysinfo`
crate for CPU and RAM metrics. All labels carry a `"system:"` provenance prefix.

| Property | Value |
|---|---|
| ID | `"system_stats"` |
| Name | `"System Stats"` |
| Scene nodes | 2 text nodes (CPU line + RAM line) |
| Update interval | Configurable, default 2 seconds |
| Content size | `(250.0, 40.4)` |
| CPU text format | `"system: CPU <pct>% · temp <celsius>°C"` or `"system: CPU <pct>% · temp: N/A"` |
| RAM text format | `"system: RAM <used>/<total> GiB"` or `"system: RAM N/A"` |

**Temperature injection**: `set_temp_source(Box<dyn FnMut() -> Option<f32> + Send>)`

The callback is invoked on every metrics refresh. Return `Some(celsius)` when a
reading is available, `None` to display `"temp: N/A"`. GLASS has zero knowledge
of hardware sensors — the consumer application injects the appropriate callback.

### FpsCounterModule (`modules/fps_counter.rs`)

Measures the **overlay's own rendering frame rate** — not the game's FPS. The
label explicitly states `"overlay-only FPS"` to avoid misleading users.

| Property | Value |
|---|---|
| ID | `"fps"` |
| Name | `"Overlay FPS"` |
| Scene nodes | 1 text node |
| Display interval | 500ms |
| Content size | `(220.0, 18.0)` |
| Ring buffer | 64 frame timestamps, zero-alloc steady state |

Call `record_frame()` after every render pass to feed timing data into the ring
buffer.

---

## Data Injection Pattern

> **Architectural decision**: GLASS does NOT have a centralized DataBus. See
> [decisions.md](decisions.md) — ADR: No DataBus in GLASS.

Each module receives its data independently. The recommended pattern is
**callback injection**:

```rust
// Consumer application wires data at construction time
let mut stats = SystemStatsModule::new();
stats.set_temp_source(Box::new(|| read_hwinfo_shared_memory()));
```

**Rationale**:
- A clock module needs no external data.
- A notification module doesn't either.
- A system stats module needs a temperature source that varies by deployment.
- A consumer application with many data sources (like Pulse) builds its own
  data layer and injects closures into each module.

This keeps GLASS generic — it never knows or cares where the data originates.
See also the [Architecture Overview](ARCHITECTURE.md) for how this fits into the
ecosystem layering.

---

## Extension Points for Custom Modules

Creating a custom module is a five-step process:

### 1. Implement `OverlayModule`

```rust
pub struct MyWidget {
    enabled: bool,
    node_ids: Vec<NodeId>,
    base_x: f32,
    base_y: f32,
    // ... your state
}

impl OverlayModule for MyWidget {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "my_widget",
            name: "My Widget",
            description: "Does something useful",
        }
    }

    fn init(&mut self, scene: &mut Scene) {
        let id = scene.add_text(TextProps { /* ... */ });
        self.node_ids.push(id);
    }

    fn update(&mut self, scene: &mut Scene, dt: Duration) -> bool {
        // Return true only if you modified the scene
        false
    }

    fn deinit(&mut self, scene: &mut Scene) {
        remove_nodes(scene, &mut self.node_ids);
    }

    fn enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    fn set_position(&mut self, x: f32, y: f32) {
        self.base_x = x;
        self.base_y = y;
    }

    fn content_size(&self) -> (f32, f32) {
        (200.0, 30.0) // Conservative estimate
    }
}
```

### 2. Inject data sources at construction

```rust
let mut widget = MyWidget::new();
widget.set_data_source(Box::new(|| fetch_my_data()));
```

### 3. Wrap in `WidgetWrapper` with desired anchor

```rust
let wrapper = WidgetWrapper::new(widget, Anchor::BottomRight, 20.0, 20.0);
```

### 4. Add to `LayoutManager`

```rust
layout.add_widget(wrapper);
```

### 5. The layout manager handles the lifecycle

`init_all` / `update_all` / `deinit_all` / `recalculate` are all driven by the
layout manager. You don't call lifecycle methods on individual modules.

---

## Helper Utilities

### `remove_nodes(scene, ids)`

```rust
pub(crate) fn remove_nodes(scene: &mut Scene, ids: &mut Vec<NodeId>) {
    for id in ids.drain(..) {
        scene.remove(id);
    }
}
```

Convenience function for `deinit` implementations. Drains the ID vector and
removes all corresponding nodes from the scene. No-op if the vector is empty.

### `BoundingBox`

Axis-aligned bounding rectangle used for hit-testing and layout calculations.
Uses half-open interval: left/top inclusive, right/bottom exclusive.

```rust
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl BoundingBox {
    pub fn contains(&self, px: f32, py: f32) -> bool;
}
```

---

## Companion Documents

| Document | Covers |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | High-level architecture, workspace crates, layer overview |
| [scene-graph.md](scene-graph.md) | Retained scene graph, node types, dirty tracking |
| [input-system.md](input-system.md) | Passive/interactive mode, HitTester, hotkey |
| [config-system.md](config-system.md) | RON/TOML loading, hot-reload, ConfigStore |
| [decisions.md](decisions.md) | Full ADR log |
