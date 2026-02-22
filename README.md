# GLASS — Windows Overlay Framework

A transparent, DPI-aware, DX12-accelerated overlay framework for Windows built on [wgpu](https://wgpu.rs/) and DirectComposition.

## Features

- **True per-pixel alpha** via DirectComposition + `DXGI_ALPHA_MODE_PREMULTIPLIED` — no faked transparency
- **wgpu DX12 backend** with a retained scene graph; re-renders only on explicit invalidation
- **Zero-allocation steady state** — no heap allocations or GPU buffer uploads when the scene is unchanged
- **Anchor-based layout** — position widgets relative to screen corners, center, or arbitrary percentages
- **Hot-reloadable config** — edit `config.ron` or `config.toml` while the overlay is running; changes apply instantly
- **Passive / interactive input modes** — default is fully click-through; a global hotkey toggles interactive mode with rect-based hit-testing
- **Module system** — composable HUD modules (`OverlayModule` trait) with init / update / deinit lifecycle
- **Built-in modules**: clock, CPU + memory stats, FPS counter
- **HDR detection** with automatic SDR fallback
- **Per-process DPI awareness** (`SetProcessDpiAwarenessContext`)
- **Anti-cheat self-check** — passive scan blocks startup if kernel-level AC is detected
- **Tracy profiling** support via optional feature flag

## Architecture

Three crates in a Cargo workspace:

```
glass-core/      — shared error types (GlassError)
glass-overlay/   — the framework library: compositor, renderer, scene, config, layout, input, modules
glass-starter/       — example application and integration harness (start here)
third_party/wgpu — git-subtree wgpu fork (patched for premultiplied alpha)
```

### Component diagram

```
glass-starter (binary)
  │
  ├─ ConfigStore ─────── RON/TOML file ──► hot-reload via notify + ArcSwap
  ├─ overlay_window ──── Win32 HWND (WS_EX_LAYERED | WS_EX_TRANSPARENT)
  ├─ Compositor ──────── IDCompositionDevice → Target → Visual
  ├─ Renderer ────────── wgpu Instance → DX12 Adapter → Device → Surface(Visual)
  │    └─ Scene ──────── retained nodes (Text, Rect) with dirty-flag tracking
  │    └─ TextEngine ─── Glyphon text rendering
  ├─ LayoutManager ───── flat widget list, anchor resolution, resize recalculation
  │    └─ WidgetWrapper ─ wraps OverlayModule + Anchor + margin
  └─ InputManager ────── passive ↔ interactive mode, visual indicator, hit-testing
```

## Quick Start

### Requirements

- Windows 10 1903+ (DirectComposition + DX12 required)
- Rust stable ≥ 1.85
- MSVC Build Tools (C++ workload — needed by `windows-sys` and `wgpu-hal`)

### Clone and run

```sh
git clone https://github.com/RomainROCH/GLASS
cd GLASS
cargo run -p glass-starter
```

A `config.ron` is created automatically with defaults on first run.

### Build all crates

```sh
cargo build --workspace
```

### Test-mode build (watermark + forced passthrough)

```sh
cargo build -p glass-starter --features test_mode
```

## Creating Your Own Module

Implement the `OverlayModule` trait from `glass_overlay::modules`:

```rust
use glass_overlay::modules::{ModuleInfo, OverlayModule};
use glass_overlay::scene::{Color, Scene, TextProps};
use std::time::Duration;

pub struct MyModule {
    enabled: bool,
    x: f32,
    y: f32,
    node_ids: Vec<glass_overlay::scene::NodeId>,
}

impl OverlayModule for MyModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "my_module",
            name: "My Module",
            description: "Shows something useful",
        }
    }

    fn init(&mut self, scene: &mut Scene) {
        let id = scene.add_text(TextProps {
            x: self.x,
            y: self.y,
            text: "Hello, overlay".into(),
            font_size: 16.0,
            color: Color::WHITE,
        });
        self.node_ids.push(id);
    }

    fn update(&mut self, scene: &mut Scene, _dt: Duration) -> bool {
        // Return true if you modified the scene (triggers re-render)
        false
    }

    fn deinit(&mut self, scene: &mut Scene) {
        for id in self.node_ids.drain(..) {
            scene.remove(id);
        }
    }

    fn enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn content_size(&self) -> (f32, f32) {
        (200.0, 20.0) // estimated bounding box for hit-testing and layout
    }
}
```

Then register it in `glass-starter/src/main.rs`:

```rust
layout_manager.add_widget(WidgetWrapper::new(
    MyModule { enabled: true, x: 0.0, y: 0.0, node_ids: vec![] },
    Anchor::TopRight,
    /*margin_x*/ 20.0,
    /*margin_y*/ 20.0,
));
```

### Lifecycle

| Call | When | Purpose |
|------|------|---------|
| `init(scene)` | Module enabled or registered | Add scene nodes |
| `update(scene, dt)` | Every tick (WM_TIMER) | Refresh content, return `true` if scene changed |
| `deinit(scene)` | Module disabled or shutdown | Remove all scene nodes |
| `set_position(x, y)` | Layout recomputation | Receive computed anchor position |
| `content_size()` | Layout + hit-testing | Return estimated `(width, height)` |

## Configuration

The overlay reads `config.ron` (or `config.toml`) from the working directory. The file is created with defaults if it does not exist. Changes are applied live — no restart needed.

```ron
// config.ron
(
    position: (x: 20.0, y: 20.0),
    size: (width: 360.0, height: 60.0),
    opacity: 1.0,
    colors: (
        primary:   (0.0, 0.0, 0.0, 0.6),   // semi-transparent black background
        secondary: (1.0, 1.0, 1.0, 1.0),   // opaque white text
    ),
    input: (
        hotkey_vk:             0x7B,   // F12 — toggle interactive mode
        hotkey_modifiers:      0,      // 0=none  1=Alt  2=Ctrl  4=Shift  8=Win
        interactive_timeout_ms: 4000,  // revert to passive after 4 s
        show_indicator:        true,   // border + label in interactive mode
    ),
    modules: (
        clock_enabled:          true,
        clock_format:           "%H:%M:%S",   // strftime syntax
        system_stats_enabled:   true,
        stats_interval_ms:      2000,
        fps_enabled:            true,
    ),
    layout: (
        clock:        (anchor: TopLeft,     margin_x: 20.0, margin_y: 20.0),
        system_stats: (anchor: TopLeft,     margin_x: 20.0, margin_y: 44.0),
        fps:          (anchor: BottomRight, margin_x: 20.0, margin_y: 20.0),
    ),
)
```

### Config field reference

| Field | Type | Description |
|-------|------|-------------|
| `position` | `{x, y}` | Initial window offset (logical pixels) |
| `size` | `{width, height}` | Overlay window dimensions (logical pixels) |
| `opacity` | `f32 [0, 1]` | Overall window opacity |
| `colors.primary` | RGBA tuple | Background / primary colour |
| `colors.secondary` | RGBA tuple | Text / secondary colour |
| `input.hotkey_vk` | `u32` | Win32 virtual key code for interactive-mode toggle |
| `input.hotkey_modifiers` | `u32` | Win32 `MOD_*` bitmask (Alt=1, Ctrl=2, Shift=4, Win=8) |
| `input.interactive_timeout_ms` | `u32` | Milliseconds before reverting to passive mode |
| `input.show_indicator` | `bool` | Show visual border + label in interactive mode |
| `modules.clock_enabled` | `bool` | Enable the clock widget |
| `modules.clock_format` | `String` | strftime-compatible format string |
| `modules.system_stats_enabled` | `bool` | Enable CPU + memory widget |
| `modules.stats_interval_ms` | `u64` | Stats refresh interval in milliseconds |
| `modules.fps_enabled` | `bool` | Enable FPS counter widget |
| `layout.<widget>.anchor` | `Anchor` | `TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`, `Center`, `ScreenPercentage(x, y)` |
| `layout.<widget>.margin_x` | `f32` | Horizontal margin from anchor point (logical pixels) |
| `layout.<widget>.margin_y` | `f32` | Vertical margin from anchor point (logical pixels) |

## Feature Flags

Feature flags are declared on `glass-starter` (or `glass-overlay` for library consumers):

| Flag | Crate | Effect |
|------|-------|--------|
| `test_mode` | `glass-overlay`, `glass-starter` | Renders a permanent watermark, forces input passthrough (no interactive mode), enables `TRACE`-level logging, prepends `[MODE TEST]` to the tray tooltip. Used during validation/testing. |
| `tracy` | `glass-overlay`, `glass-starter` | Wires `tracing` spans into the [Tracy](https://github.com/wolfpld/tracy) profiler via `tracing-tracy`. Requires a running Tracy server. |
| `alloc-tracking` | `glass-starter` | Installs a debug allocator that counts heap allocations. Logs allocation counts at startup and can be used to verify zero-allocation steady state. |
| `gaming` | `glass-starter`, `glass-overlay`, `glass-core` | Enables optional gaming safety checks (anti-cheat self-check). Non-gaming consumers can leave it disabled. |

```sh
# Build with Tracy profiling
cargo build -p glass-starter --features tracy

# Build test-mode binary
cargo build -p glass-starter --features test_mode

# Combine flags
cargo build -p glass-starter --features "test_mode,alloc-tracking"
```

## Technical Deep-Dive

### Why the wgpu fork?

`wgpu-hal` 24.x hardcodes `DXGI_ALPHA_MODE_IGNORE` in `CreateSwapChainForComposition`, making the swapchain opaque regardless of the `alpha_mode` field in `SurfaceConfiguration`. The overlay's premultiplied alpha blending requires `DXGI_ALPHA_MODE_PREMULTIPLIED`.

GLASS ships a git-subtree copy of wgpu at `third_party/wgpu/` that patches `wgpu-hal` and `wgpu-core` so composition swapchains honour and expose `CompositeAlphaMode::PreMultiplied`. `wgpu-types` and `naga` are also included to ensure the entire set resolves to identical types (avoiding "multiple versions of the same crate" linker errors). The `[patch.crates-io]` section in the root `Cargo.toml` activates these overrides transparently.

### DirectComposition pipeline

Standard HWND-based DX12 swapchains expose only `DXGI_ALPHA_MODE_IGNORE`. DirectComposition (`IDCompositionDevice`) provides an alternative composition path:

```
DCompositionCreateDevice()
  └─ CreateTargetForHwnd(hwnd)        ← attaches composition to window
       └─ CreateVisual()              ← a composition node
            └─ SetRoot(visual)
                 └─ wgpu::SurfaceTargetUnsafe::CompositionVisual(visual_ptr)
                      └─ CreateSwapChainForComposition  ← PREMULTIPLIED alpha works here
```

After `wgpu` configures the surface (which calls `SetContent` internally to bind the swapchain to the visual), `IDCompositionDevice::Commit()` is called once to make the binding take effect. After that, normal `wgpu` present calls drive frame submission.

### Retained rendering and zero-allocation steady state

The `Scene` graph holds all visible nodes (text, rects). Nodes carry a dirty flag. The render path checks `scene.is_dirty()` before re-uploading anything to the GPU. In steady state — no module updates, no config changes — a render tick performs:

1. `get_current_texture()` — swap-chain acquire
2. `begin_render_pass` with `LoadOp::Clear(transparent)` — clears to (0,0,0,0)
3. `text_engine.render()` — Glyphon submits pre-built atlas draw calls (no re-layout)
4. `queue.submit()` + `frame.present()`
5. `scene.clear_dirty()` — reset flags

No `Vec` allocations, no string formatting, no GPU buffer writes beyond the draw submission itself.

### Premultiplied alpha in shaders

All fragment shaders must output premultiplied RGBA (i.e. `rgb *= alpha` before writing). The blend state is configured as `(One, OneMinusSrcAlpha)` on both color and alpha channels. Writing straight alpha values will produce incorrect compositing artifacts.

```wgsl
// Correct — premultiplied green at 50% opacity
out.color = vec4<f32>(0.0, 0.5, 0.0, 0.5);

// Wrong — straight alpha (DO NOT USE with DirectComposition)
// out.color = vec4<f32>(0.0, 1.0, 0.0, 0.5);
```

## Building

```sh
# Full workspace
cargo build --workspace

# Release
cargo build --workspace --release

# Starter harness only
cargo build -p glass-starter

# Run tests
cargo test --workspace
```

**Windows-only.** The codebase uses `windows-rs` APIs (`IDCompositionDevice`, `DWM`, `Win32_UI_HiDpi`, etc.) that have no cross-platform equivalents. Compilation on Linux/macOS will fail.

**CI**: GitHub Actions runs `cargo build --workspace` and `cargo test --workspace` on `windows-latest`.

## License

Licensed under MIT or Apache 2.0, at your option. See LICENSE-MIT and LICENSE-APACHE for details.
