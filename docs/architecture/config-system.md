# Configuration System

← [Architecture Overview](ARCHITECTURE.md)

---

## Design

GLASS uses a hot-reloadable configuration system supporting **RON** and **TOML**
formats. The system is built on filesystem watching (`notify`) and lock-free
reads (`arc-swap`) to ensure the render loop is never blocked by configuration
access.

**Source:** `glass-overlay/src/config.rs`  
**Supporting types:** `glass-overlay/src/layout.rs` (`LayoutConfig`, `WidgetLayoutConfig`, `Anchor`), `glass-overlay/src/modules/mod.rs` (`ModulesConfig`)

---

## ConfigStore

`ConfigStore` is the thread-safe configuration store at the center of the
system. It holds the current `OverlayConfig` in an `ArcSwap` and optionally
watches the source file for changes.

```rust
pub struct ConfigStore {
    inner: Arc<ArcSwap<OverlayConfig>>,
    path: PathBuf,
    format: ConfigFormat,
    _watcher: Mutex<Option<RecommendedWatcher>>,
}
```

### API

| Method | Signature | Behavior |
|---|---|---|
| `load(path)` | `ConfigStore::load<P: AsRef<Path>>(path: P) -> Result<Self, GlassError>` | Loads RON or TOML based on file extension. If the file does not exist, writes a default config and logs a message. Validates and clamps out-of-range values on load. |
| `watch()` | `&self -> Result<(), GlassError>` | Spawns a background thread with `notify::RecommendedWatcher`. Watches the **parent directory** (to handle atomic editor saves via temp+rename). On file change, parses the new config and atomically swaps it in. On parse failure, the previous config is kept and an error is logged. |
| `get()` | `&self -> arc_swap::Guard<Arc<OverlayConfig>>` | Returns the current config snapshot via `ArcSwap::load()`. **Lock-free, zero allocations.** Safe to call from the render thread every frame. The returned `Guard` keeps the `Arc` alive even if a concurrent reload swaps in a new value. |

### Thread Safety

`ArcSwap` is the key primitive:

- **Readers** (render thread) call `get()` → `ArcSwap::load()` — lock-free,
  wait-free, no allocation. Multiple readers can proceed concurrently.
- **Writer** (watcher thread) calls `ArcSwap::store(Arc::new(new_config))` —
  atomically replaces the pointer. Existing readers still hold valid `Arc`s to
  the previous config until they drop them.

No `Mutex` is involved in the read path. The only `Mutex` in `ConfigStore`
guards the watcher handle lifetime, not the config data.

---

## Hot-Reload Architecture

```
Config File (RON/TOML)
    │
    ▼
notify::RecommendedWatcher (background thread)
    │  watches parent directory (handles atomic saves)
    │  filters events by file name
    │  50ms debounce (editors trigger multiple events)
    │
    ▼
parse_config(content, format, path)
    │  validates + clamps out-of-range values
    │  on parse failure → log error, keep previous config
    │
    ▼
ArcSwap::store(Arc::new(new_config))
    │  atomically swaps the pointer
    │  old config stays alive until all readers drop it
    │
    ▼
Render Thread: config_store.get() → Arc<OverlayConfig>
    (lock-free read on every frame)
```

### Important Caveat

`watch()` updates the stored config snapshot, but **the running application
must still re-read and reapply the snapshot** for behavior to change. The
`ConfigStore` signals availability — it does not trigger automatic application
of new settings.

The reference starter (`glass-starter/src/main.rs`) reads config once at
startup and does not currently re-read or reapply after hot-reload. This is
documented intentionally — it is the consumer application's responsibility to
decide when and how to react to config changes.

The pattern for a consumer that wants live reloading:

```rust
// In the render/update loop:
let cfg = config_store.get();
if cfg.opacity != current_opacity {
    // Reapply opacity to the overlay
    current_opacity = cfg.opacity;
}
```

---

## OverlayConfig Structure

The root configuration type. All fields have defaults and are validated on load.

```rust
pub struct OverlayConfig {
    pub position: Position,       // Initial overlay window position
    pub size: Size,               // Overlay window dimensions
    pub opacity: f32,             // 0.0–1.0, clamped on load
    pub colors: Colors,           // Overlay color palette
    pub input: InputConfig,       // Hotkey, timeout, indicator
    pub modules: ModulesConfig,   // Per-module enable/format/interval
    pub layout: LayoutConfig,     // Per-widget anchor/margin
}
```

### Validation

`OverlayConfig::validate()` runs after every parse (initial load and reload):

- `opacity` outside `[0.0, 1.0]` → clamped, logged as warning
- `size.width` or `size.height` ≤ 0.0 → clamped to 1.0, logged as warning

### Diff Summary

`OverlayConfig::diff_summary(&self, other: &Self)` produces a human-readable
string of what changed between two configs. Used in reload logging:

```
Config reloaded: opacity: 1.00 -> 0.80, position: (20,20) -> (50,50)
```

---

## Supporting Types

### Position

```rust
pub struct Position {
    pub x: f32,  // Horizontal coordinate in logical pixels
    pub y: f32,  // Vertical coordinate in logical pixels
}
// Default: { x: 20.0, y: 20.0 }
```

### Size

```rust
pub struct Size {
    pub width: f32,   // Width in logical pixels
    pub height: f32,  // Height in logical pixels
}
// Default: { width: 360.0, height: 60.0 }
```

### Rgba

```rust
pub struct Rgba(pub f32, pub f32, pub f32, pub f32);
// Default: (1.0, 1.0, 1.0, 1.0) — opaque white
```

RGBA color with four `f32` components in `[0.0, 1.0]`.

### Colors

```rust
pub struct Colors {
    pub primary: Rgba,    // Background / primary color
    pub secondary: Rgba,  // Text / secondary color
}
// Default: primary=(0.0, 0.0, 0.0, 0.6), secondary=(1.0, 1.0, 1.0, 1.0)
```

### InputConfig

```rust
pub struct InputConfig {
    pub hotkey_vk: u32,              // Virtual key code (default: 0x7B = F12)
    pub hotkey_modifiers: u32,       // Win32 MOD_* bitmask (default: 0 = none)
    pub interactive_timeout_ms: u32, // Timeout before reverting to passive (default: 4000)
    pub show_indicator: bool,        // Show border+label in interactive mode (default: true)
}
```

Modifier flags: `1` = Alt, `2` = Ctrl, `4` = Shift, `8` = Win. Combine with bitwise OR.

### ModulesConfig

Defined in `glass-overlay/src/modules/mod.rs`:

```rust
pub struct ModulesConfig {
    pub clock_enabled: bool,          // default: true
    pub clock_format: String,         // default: "%H:%M:%S" (strftime)
    pub system_stats_enabled: bool,   // default: true
    pub stats_interval_ms: u64,       // default: 2000
    pub fps_enabled: bool,            // default: true
}
```

### LayoutConfig

Defined in `glass-overlay/src/layout.rs`:

```rust
pub struct LayoutConfig {
    pub clock: WidgetLayoutConfig,
    pub system_stats: WidgetLayoutConfig,
    pub fps: WidgetLayoutConfig,
}

pub struct WidgetLayoutConfig {
    pub anchor: Anchor,     // TopLeft, TopRight, BottomLeft, BottomRight, Center, ScreenPercentage(f32, f32)
    pub margin_x: f32,     // Horizontal offset from anchor edge (pixels)
    pub margin_y: f32,     // Vertical offset from anchor edge (pixels)
}
```

Default layout stacks all widgets at the top-left with increasing `margin_y`:
clock at `(10, 10)`, system stats at `(10, 34)`, FPS at `(10, 60)`.

---

## Format Support

| Format | Extension | Library | Notes |
|---|---|---|---|
| RON | `.ron` | `ron` | Default format. More expressive (enums, tuples). Used by `config.ron`. |
| TOML | `.toml` | `toml` | Alternative. More familiar to users coming from Cargo/Python tooling. |

Format is detected at load time by file extension via `detect_format()`. Unknown
or missing extensions produce a `GlassError::ConfigError`.

Both formats use the same `serde` `Serialize`/`Deserialize` derives on all
config types. This means any config that works in RON also works in TOML
(modulo syntax differences), and vice versa.

---

## Reference Config

The repository ships `config.ron` at the workspace root as the reference
configuration used by `glass-starter`:

```ron
(
    position: (x: 20.0, y: 20.0),
    size: (width: 360.0, height: 60.0),
    opacity: 1.0,
    colors: (
        primary: (0.0, 0.0, 0.0, 0.6),
        secondary: (1.0, 1.0, 1.0, 1.0),
    ),
    input: (
        hotkey_vk: 0x7B,
        hotkey_modifiers: 0,
        interactive_timeout_ms: 4000,
        show_indicator: true,
    ),
    modules: (
        clock_enabled: true,
        clock_format: "%H:%M:%S",
        system_stats_enabled: true,
        stats_interval_ms: 2000,
        fps_enabled: true,
    ),
    layout: (
        clock: (anchor: TopLeft, margin_x: 10.0, margin_y: 10.0),
        system_stats: (anchor: TopLeft, margin_x: 10.0, margin_y: 34.0),
        fps: (anchor: TopLeft, margin_x: 10.0, margin_y: 60.0),
    ),
)
```

---

## Extension Pattern for Custom Apps

GLASS's `OverlayConfig` covers the framework's built-in settings. Custom
applications that need additional configuration (e.g. per-game sensor
settings, IPC endpoints, custom module parameters) should keep their own
config alongside — not extend `OverlayConfig`.

Recommended pattern:

```rust
// 1. Load GLASS's built-in config
let glass_store = ConfigStore::load("config.ron")?;
glass_store.watch()?;

// 2. Load app-specific config separately
let app_config: MyAppConfig = load_my_config("pulse.toml")?;

// 3. Pass custom settings into module constructors
let mut stats = SystemStatsModule::new();
stats.set_temp_source(build_sensor_callback(&app_config));

// 4. Use GLASS config for framework concerns (position, size, layout)
let cfg = glass_store.get();
let layout_manager = LayoutManager::new(cfg.size.width, cfg.size.height);
```

This separation keeps GLASS generic — the framework config covers window
placement, colors, modules, and layout. Application-specific concerns
(sensor sources, game profiles, custom module data) stay in the consumer's
config layer.

---

## Companion Documents

| Document | Covers |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | High-level system architecture and layer overview |
| [decisions.md](decisions.md) | Architecture Decision Records |
| [module-system.md](module-system.md) | `OverlayModule` trait, `ModuleRegistry`, layout anchoring |
| [input-system.md](input-system.md) | Passive/interactive mode, hotkey, timeout |
