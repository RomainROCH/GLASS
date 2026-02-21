# GLASS — Ultimate Overlay

GLASS is a transparent, click-through overlay system for Windows, built on DirectComposition and wgpu/DX12.

## Architecture

This project is structured as a Cargo workspace with three crates, following a layered architecture from core types through overlay implementation to the executable harness.

### Workspace Structure

```
glass-core/        → Core types and errors
glass-overlay/     → Overlay implementation
glass-poc/         → Executable binary (PoC harness)
```

### Crate Responsibilities

#### `glass-core`

**Purpose**: Foundational types and error definitions shared across the workspace.

**Location**: `glass-core/src/`

**Contents**:
- `error.rs` — `GlassError` enum covering DirectComposition init, wgpu init, window creation, HDR detection, config errors, input errors, OS errors, and anti-cheat safety blocks

**Dependencies**:
- `tracing` (workspace)
- Zero external platform dependencies

**Role in workspace**: Provides the top-level error type (`GlassError`) used by both `glass-overlay` and `glass-poc`. Acts as a minimal common foundation without pulling in platform-specific or rendering dependencies.

---

#### `glass-overlay`

**Purpose**: Production overlay library implementing the full transparent overlay stack.

**Location**: `glass-overlay/src/`

**Public API modules** (exposed via `lib.rs`):
- `compositor` — DirectComposition device/target/visual management (`Compositor` type)
- `renderer` — wgpu DX12 rendering backend (`Renderer` type)
- `overlay_window` — HWND creation, DPI awareness, system tray icon, Win32 message pump
- `scene` — Retained scene graph with dirty-flag tracking for efficient re-rendering
- `text_renderer` — Glyphon text rendering integration
- `config` — Hot-reloadable configuration (RON/TOML) via `ConfigStore`
- `input` — Passive/interactive mode switching with hotkey support (`InputManager`)
- `hdr` — HDR detection with SDR fallback
- `layout` — Widget layout and positioning system (`LayoutManager`)
- `modules` — Built-in overlay widgets (clock, FPS counter, system stats)
- `diagnostics` — System diagnostics dump on errors
- `safety` — Anti-cheat detection (`AntiCheatDetector`, `DetectionPolicy`)
- `test_mode` — Test build constants (watermark labels, forced passthrough flag)

**Key types**:
- `Compositor` — Wraps `IDCompositionDevice`, `IDCompositionTarget`, `IDCompositionVisual` (Windows DirectComposition APIs)
- `Renderer` — Owns wgpu `Instance`, `Device`, `Queue`, `Surface`, manages render pipeline and per-frame draws
- `OverlayWindow` (module-level functions) — Creates layered, click-through HWND with `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOREDIRECTIONBITMAP`

**Dependencies**:
- `glass-core` (workspace) — for `GlassError`
- `wgpu` (workspace, `dx12` feature) — rendering backend
- `windows` (workspace) — Win32, DirectComposition, DXGI, DWM APIs
- `raw-window-handle` — window handle interop
- `glyphon` — text rendering
- `sysinfo` — system metrics (CPU/memory)
- `chrono` — time formatting
- `serde`, `ron`, `toml`, `serde_json` — config serialization
- `arc-swap`, `notify` — config hot-reload
- `tracing`, `tracing-subscriber`, `tracing-tracy` (optional) — logging and profiling

**Features**:
- `test_mode` — Renders permanent watermark, forces input passthrough, enables TRACE-level logging, prepends `[MODE TEST]` to tray tooltip (used for anti-cheat validation campaigns)
- `tracy` — Opt-in Tracy profiler integration

**Platform specificity**:
- **Windows-only**: Uses DirectComposition (`IDCompositionDevice`, `IDCompositionTarget`, `IDCompositionVisual`), Win32 windowing APIs (`HWND`, `CreateWindowExW`, `GetClientRect`, message loop), DPI awareness (`SetProcessDpiAwarenessContext`), system tray (`Shell_NotifyIconW`)
- **DX12-only**: wgpu backend hardcoded to `wgpu::Backends::DX12`, uses `wgpu::SurfaceTargetUnsafe::CompositionVisual` for DirectComposition binding
- **wgpu fork dependency**: Requires patched `wgpu-hal` and `wgpu-types` from `third_party/wgpu` for premultiplied alpha support (`wgpu::CompositeAlphaMode::PreMultiplied` + `DXGI_ALPHA_MODE_PREMULTIPLIED`) on DirectComposition surfaces

**Role in workspace**: Implements all core overlay functionality as a library. Provides the public API for creating and managing overlays. `glass-poc` consumes this API without duplicating logic.

---

#### `glass-poc`

**Purpose**: Executable binary that exercises `glass-overlay` as a proof-of-concept harness.

**Location**: `glass-poc/src/`

**Contents**:
- `main.rs` — Lifecycle bootstrap: tracing init (+ Tracy if enabled), anti-cheat self-check, DPI awareness, config load + hot-reload watcher, window + DirectComposition + wgpu init, module registry setup (clock, system stats, FPS counter), Win32 message loop with retained rendering + module ticks
- `alloc_tracker.rs` — Optional allocation tracking (via `alloc-tracking` feature)

**Dependencies**:
- `glass-core` (workspace) — for `GlassError`
- `glass-overlay` (workspace) — all overlay functionality
- `tracing`, `tracing-subscriber`, `tracing-tracy` (optional) — logging

**Features**:
- `alloc-tracking` — Enables custom allocation tracking
- `test_mode` — Propagates to `glass-overlay/test_mode`
- `tracy` — Propagates to `glass-overlay/tracy` + adds `tracing-tracy` integration

**Role in workspace**: Thin harness that proves the viability of the overlay (wgpu DX12 + transparent HWND + click-through). All core logic resides in `glass-overlay`; `glass-poc` is a minimal bootstrap shim. Input modes: passive (default) ↔ interactive (hotkey toggle). In `test_mode` builds, interactive mode is forcibly disabled.

---

### Workspace Dependency Graph

```
glass-poc (bin)
  ├─> glass-overlay (lib)
  │     └─> glass-core (lib)
  └─> glass-core (lib)
```

**Workspace-level dependencies** (defined in root `Cargo.toml`):
- **Rendering**: `wgpu` (v24, `dx12` feature only)
- **Windows APIs**: `windows` (v0.59, features: DirectComposition, DWM, DXGI, Direct3D12, Win32 windowing, input, shell, HiDPI)
- **Text rendering**: `glyphon` (v0.8)
- **Config**: `serde`, `ron`, `toml`, `serde_json`, `arc-swap` (hot-reload), `notify` (file watcher)
- **System metrics**: `sysinfo` (v0.33)
- **Time**: `chrono` (v0.4)
- **Logging/profiling**: `tracing`, `tracing-subscriber`, `tracing-tracy` (optional)
- **Async**: `pollster` (for wgpu init)
- **Interop**: `raw-window-handle` (v0.6)

**Local patches** (via `[patch.crates-io]`):
- `wgpu-hal`, `wgpu-types`, `naga` — Patched from `third_party/wgpu` to add premultiplied alpha support for DirectComposition surfaces (see `sync_wgpu.py` workflow)

---

### Design Principles

1. **Separation of concerns**: `glass-core` provides shared types, `glass-overlay` implements all overlay logic, `glass-poc` is a minimal harness.
2. **Library-first**: Core functionality lives in `glass-overlay` as a reusable library, not embedded in the binary.
3. **Platform abstraction boundary**: Currently Windows-only; platform-specific code is isolated in `glass-overlay` modules (`compositor.rs`, `overlay_window.rs`, `renderer.rs` Windows/DX12 paths). The isolation design enables alternative implementations behind a common API pattern.
4. **Feature flags for validation**: `test_mode` enables anti-cheat validation artifacts (watermarks, forced passthrough) without modifying production code paths.
5. **Dependency minimization in core**: `glass-core` has zero platform dependencies, making it a stable base for error handling across the workspace.

---

## Platform Boundaries

### Current Platform Support

GLASS is **Windows-only** in its current implementation. The transparency mechanism relies on Windows-specific APIs that do not have direct equivalents on other platforms.

### Windows-Specific Components

The following components are tightly coupled to Windows and would require platform-specific replacements:

#### **DirectComposition (Critical Path)**
- **Location**: `glass-overlay/src/compositor.rs`
- **APIs**: `IDCompositionDevice`, `IDCompositionTarget`, `IDCompositionVisual`, `DCompositionCreateDevice`
- **Purpose**: Enables per-pixel alpha transparency via composition swapchains with `DXGI_ALPHA_MODE_PREMULTIPLIED`
- **Why it matters**: HWND-based DX12 swapchains only support `DXGI_ALPHA_MODE_IGNORE` (opaque). DirectComposition is the **only** Windows API that provides true transparent overlays.
- **Linux/macOS equivalent** (theoretical, not implemented):
  - **Linux**: Wayland subsurfaces with alpha or X11 compositing (requires Wayland compositor support or X11 ARGB visuals)
  - **macOS**: `CALayer` with transparent backing or `NSWindow` with `NSWindowStyleMaskBorderless` + alpha channel

#### **Win32 Windowing**
- **Location**: `glass-overlay/src/overlay_window.rs`
- **APIs**: `CreateWindowExW`, `WS_EX_LAYERED`, `WS_EX_TRANSPARENT`, `WS_EX_NOREDIRECTIONBITMAP`, `SetLayeredWindowAttributes`, `GetMessageW`, `DispatchMessageW`
- **Purpose**: Creates a layered, click-through window with DPI awareness and system tray integration
- **Linux/macOS equivalent** (theoretical, not implemented):
  - **Linux**: Wayland `xdg_surface` with input region control, or X11 `XShapeCombineRectangles` for click-through
  - **macOS**: `NSWindow` with `ignoresMouseEvents` and `setAcceptsMouseMovedEvents`

#### **DX12 Rendering Backend**
- **Location**: `glass-overlay/src/renderer.rs` (line 139: `wgpu::Backends::DX12`)
- **APIs**: wgpu configured for DX12-only, uses `SurfaceTargetUnsafe::CompositionVisual`
- **Purpose**: GPU rendering with premultiplied alpha support (via patched wgpu)
- **Linux/macOS equivalent** (theoretical, not implemented):
  - **Linux**: Vulkan backend with `VK_COMPOSITE_ALPHA_PRE_MULTIPLIED_BIT_KHR` on Wayland
  - **macOS**: Metal backend with `CAMetalLayer` and premultiplied alpha blending

#### **System Tray Icon**
- **Location**: `glass-overlay/src/overlay_window.rs` (lines 193-220)
- **APIs**: `Shell_NotifyIconW`, `NIM_ADD`, `NIM_DELETE`, `NIM_MODIFY`
- **Purpose**: System tray icon with tooltip and context menu
- **Linux/macOS equivalent** (theoretical, not implemented):
  - **Linux**: StatusNotifier/AppIndicator (freedesktop.org standard) or legacy XEmbed tray
  - **macOS**: `NSStatusBar` + `NSStatusItem`

#### **DPI Awareness**
- **Location**: `glass-overlay/src/overlay_window.rs` (lines 40-52)
- **APIs**: `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)`
- **Purpose**: Per-monitor DPI scaling on multi-monitor setups
- **Linux/macOS equivalent** (theoretical, not implemented):
  - **Linux**: Wayland scaling factors or X11 Xft.dpi
  - **macOS**: `NSScreen.backingScaleFactor` (Retina detection)

#### **HDR Detection**
- **Location**: `glass-overlay/src/hdr.rs`
- **APIs**: `IDXGIFactory1`, `IDXGIOutput6`, `GetColorSpace`, `DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020`
- **Purpose**: Detects HDR-capable displays and selects appropriate surface format
- **Linux/macOS equivalent** (theoretical, not implemented):
  - **Linux**: Wayland HDR metadata protocols (in development)
  - **macOS**: `NSScreen.maximumExtendedDynamicRangeColorComponentValue`

#### **Input Mode Switching**
- **Location**: `glass-overlay/src/input.rs`, `glass-overlay/src/overlay_window.rs`
- **APIs**: `SetWindowLongPtrW`, `RegisterHotKey`, `UnregisterHotKey`, `SetTimer`, `KillTimer`
- **Purpose**: Toggle between passive (click-through) and interactive modes via hotkey
- **Linux/macOS equivalent** (theoretical, not implemented):
  - **Linux**: X11 input regions or Wayland input/focus protocols
  - **macOS**: `NSWindow.ignoresMouseEvents` toggling

#### **Anti-Cheat Detection**
- **Location**: `glass-overlay/src/safety.rs` (lines 192-264)
- **APIs**: `OpenSCManagerW`, `OpenServiceW`, `CreateToolhelp32Snapshot`, `Process32FirstW/NextW`
- **Purpose**: Detects active anti-cheat services/drivers (Vanguard, Ricochet, EAC, BattlEye, VAC)
- **Linux/macOS equivalent** (theoretical, not implemented):
  - **Linux**: `/proc` filesystem parsing, `systemctl` service queries
  - **macOS**: `libproc` process enumeration, `launchctl` service queries

### Cross-Platform Components

The following components are **already platform-agnostic** and would work on Linux/macOS without modification:

- **Scene Graph** (`glass-overlay/src/scene.rs`) — Pure Rust data structures with dirty-flag tracking
- **Layout System** (`glass-overlay/src/layout.rs`) — Anchor-based positioning math (no OS APIs)
- **Module System** (`glass-overlay/src/modules/`) — Generic trait + implementations using cross-platform deps:
  - Clock module: `chrono` crate (cross-platform)
  - System stats module: `sysinfo` crate (Linux/macOS/Windows support)
  - FPS counter: Pure frame timing math
- **Configuration** (`glass-overlay/src/config.rs`) — RON/TOML parsing via `serde` (except VK codes for hotkeys)
- **Text Rendering** (`glass-overlay/src/text_renderer.rs`) — Glyphon integration (works on any wgpu backend)

### Porting Strategy (For Future Work)

To support Linux or macOS, the following architectural changes would be required:

1. **Create platform abstraction traits**:
   - `trait CompositorBackend` — Abstract DirectComposition (Windows), Wayland subsurfaces (Linux), CALayer (macOS)
   - `trait WindowBackend` — Abstract Win32 windowing, X11/Wayland, Cocoa
   - `trait InputBackend` — Abstract hotkey registration and click-through toggling

2. **Parameterize wgpu backend selection**:
   - Replace hardcoded `Backends::DX12` with runtime selection (Vulkan for Linux, Metal for macOS)
   - Ensure premultiplied alpha support on all backends (Vulkan and Metal both support it natively)

3. **Replace platform-specific config fields**:
   - Convert `hotkey_vk: u32` (Win32 virtual key codes) to cross-platform key enum (e.g., `winit::event::VirtualKeyCode`)

4. **Stub or reimplement OS-specific features**:
   - Anti-cheat detection: Platform-specific implementations or compile-time stubs
   - HDR detection: Per-platform APIs or assume SDR fallback

**Estimated effort**: 2-4 weeks for Linux/Wayland support, 1-2 weeks for macOS/Cocoa support (assuming premultiplied alpha is already working in wgpu's Vulkan/Metal backends).

---

## Fork and Adoption Guidance

GLASS can be adopted in two practical ways today: as a standalone fork (`glass-poc` as app entrypoint) or as an embedded library integration (`glass-overlay` reused inside another app).

### Adoption Mode 1: Standalone Application Fork

Use this mode if you want to ship your own overlay executable based on this repo.

Recommended edits:
- Keep the rendering/composition core as-is:
  - `glass-overlay/src/compositor.rs`
  - `glass-overlay/src/renderer.rs`
  - `third_party/wgpu/` + `[patch.crates-io]`
- Customize app behavior in:
  - `glass-poc/src/main.rs` (bootstrap flow, enabled modules, startup policy)
  - `config.ron` (defaults for position/size/colors/input/modules/layout)
  - `glass-overlay/src/modules/` (replace/add widgets)
- Remove PoC-only behavior if not needed:
  - anti-cheat startup gate (`glass-overlay/src/safety.rs` call path from `glass-poc/src/main.rs`)
  - demo hit-test rectangle (`glass-poc/src/main.rs`)
  - test-mode watermark path (`glass-overlay/src/test_mode.rs`, `renderer.rs`)

Fork maintenance:
- Keep subtree patches synchronized with:
  - `./python sync_wgpu.py status`
  - `./python sync_wgpu.py pull`
  - `./python sync_wgpu.py push`

### Adoption Mode 2: Embedded Integration

Use this mode if you already have an app and want to integrate GLASS capabilities incrementally.

Integration boundaries in current code:
- `glass-overlay` already separates core concerns:
  - Windowing: `overlay_window.rs`
  - Composition: `compositor.rs`
  - Rendering: `renderer.rs`
  - Scene/text: `scene.rs`, `text_renderer.rs`
  - Modules/layout/config: `modules/*`, `layout.rs`, `config.rs`
- The existing harness sequence lives in `glass-poc/src/main.rs`:
  1) DPI awareness
  2) config load/watch
  3) window creation
  4) DirectComposition init
  5) renderer init
  6) module/layout wiring
  7) retained message/render loop

What to expect when embedding:
- This implementation creates and owns its own overlay HWND path (Win32-specific).
- Event loop and hotkey behavior are tied to Win32 message processing.
- The transparency path depends on the patched wgpu subtree and must be carried with the integration.

---

## Rendering Pipeline

GLASS uses a multi-stage initialization flow that chains Windows DirectComposition with wgpu/DX12 rendering:

### 1. Window Creation
`glass-overlay/src/overlay_window.rs` — Layered, click-through HWND:
- Extended window styles: `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOREDIRECTIONBITMAP`
- `WS_EX_NOREDIRECTIONBITMAP` enables DirectComposition backing (bypasses legacy DWM bitmap redirection)
- DPI awareness set via `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)`
- System tray icon + message loop integration

### 2. DirectComposition Setup
`glass-overlay/src/compositor.rs` — True per-pixel alpha transparency:
- Creates `IDCompositionDevice` via `DCompositionCreateDevice`
- Creates `IDCompositionTarget` bound to the HWND
- Creates `IDCompositionVisual` and sets it as the target's root visual
- Returns a raw pointer to the visual for wgpu binding

**Why DirectComposition?** HWND-based DX12 swapchains only support `DXGI_ALPHA_MODE_IGNORE` (opaque). DirectComposition's composition swapchains support `DXGI_ALPHA_MODE_PREMULTIPLIED`, enabling real transparency.

### 3. wgpu Surface + Configuration
`glass-overlay/src/renderer.rs` — DX12 backend initialization:
- **Instance**: `wgpu::Instance::new()` with `Backends::DX12` (hardcoded; other backends unsupported)
- **Surface**: `instance.create_surface_unsafe(SurfaceTargetUnsafe::CompositionVisual(visual_ptr))` binds the wgpu surface to the DirectComposition visual
- **Adapter**: Requests a DX12-compatible adapter with `compatible_surface`
- **Device/Queue**: Device and command queue creation
- **Surface configuration**:
  - Format selection: HDR-capable (`Rgba16Float` + scRGB) or SDR fallback (`Bgra8UnormSrgb`)
  - Alpha mode: `CompositeAlphaMode::PreMultiplied` (requires patched wgpu — see below)
  - Present mode: `Mailbox` (low-latency, adaptive vsync)
- **Commit**: `Compositor::commit()` applies the visual → swapchain binding to DirectComposition

### 4. Scene + Text Rendering
Retained rendering model with dirty-flag tracking:

**Scene Graph** (`glass-overlay/src/scene.rs`):
- Nodes: `Rect` (solid-color rectangles), `Text` (glyphon text)
- Each node has a unique `NodeId` and dirty flag
- Modifications (`add`, `update`, `remove`) mark the scene as dirty
- `scene.clear_dirty()` is called after each successful render

**Text Engine** (`glass-overlay/src/text_renderer.rs`):
- Wraps `glyphon::TextRenderer` for GPU-accelerated text
- Prepares glyphon text atlas on each frame (only re-uploads dirty glyphs)
- Renders text during the wgpu render pass

**Render Pass** (`renderer::render()`):
- Clears framebuffer to transparent black `(0, 0, 0, 0)`
- Draws PoC triangle (premultiplied green at 50% alpha)
- Draws scene text nodes via `text_engine.render()`
- Presents frame via `frame.present()`
- Surface error recovery: reconfigures surface on `Lost`/`Outdated` and retries once

### 5. Retained Redraw Triggers
Rendering is **on-demand** (not continuous) — frames are only rendered when invalidated:

**Explicit triggers**:
- **Scene mutation** — `scene.add_*()`, `scene.update()`, `scene.remove()` set dirty flags
- **Module updates** — Timer-based module ticks (system stats, clock, FPS counter) mutate scene text nodes, triggering redraws
- **Config hot-reload** — File watcher detects `config.ron` changes, reloads config, updates modules, invalidates scene
- **Input mode toggle** — Hotkey switches between passive/interactive modes, renders input indicator overlay
- **Window resize** — `WM_SIZE` message reconfigures surface, triggers redraw

**Message loop** (`glass-poc/src/main.rs` + `overlay_window::run_message_loop`):
- Pumps Win32 messages (`PeekMessageW` + `DispatchMessageW`)
- Processes `WM_TIMER` for periodic module updates (default: 1000ms interval)
- Calls `renderer.render()` only when scene is dirty or module ticks mutate state

---

## wgpu Vendoring + Patching Workflow

### Why Vendor wgpu?
**Problem**: wgpu 24.0.4 from crates.io hardcodes `CompositeAlphaMode::Opaque` and `DXGI_ALPHA_MODE_IGNORE` for DirectComposition surfaces. This prevents true transparency.

**Solution**: Vendor a patched wgpu subtree that adds `PreMultiplied` support for `SurfaceTargetUnsafe::CompositionVisual` targets.

### Subtree Location
`third_party/wgpu` — Git subtree tracking the `v24` branch of `https://github.com/RomainROCH/wgpu.git` (fork of gfx-rs/wgpu)

### Cargo Patch Configuration
`Cargo.toml` uses `[patch.crates-io]` to redirect wgpu-hal and wgpu-types to the vendored subtree:

```toml
[patch.crates-io]
wgpu-hal   = { path = "third_party/wgpu/wgpu-hal" }
wgpu-types = { path = "third_party/wgpu/wgpu-types" }
naga       = { path = "third_party/wgpu/naga" }
```

**Why patch wgpu-types?** `wgpu-core` (from crates.io) and `wgpu-hal` (from `third_party/`) both depend on `wgpu-types`. Without the patch, Cargo resolves two different `wgpu-types` crate instances, causing duplicate type errors.

### sync_wgpu.py Workflow
`sync_wgpu.py` — Python script for managing the wgpu subtree lifecycle:

**Commands**:
- `setup` — Adds the `wgpu-fork` remote and performs the initial subtree adoption (or fresh add)
- `pull` — Fetches upstream wgpu changes from the fork (`v24` branch) and merges them into `third_party/wgpu` (squashed)
- `push` — Pushes local wgpu-hal patches from `third_party/wgpu` back to the fork
- `status` — Shows current remote state and subtree merge history

**Usage**:
```bash
# Initial setup (first-time clone)
./python sync_wgpu.py setup

# Pull upstream updates from fork
./python sync_wgpu.py pull

# Push local patches to fork
./python sync_wgpu.py push

# Check subtree status
./python sync_wgpu.py status
```

**Workflow guarantees**:
- Aborts on dirty working tree (prevents merge conflicts)
- Squashes subtree merge commits (clean history)
- Automates remote configuration (sets `wgpu-fork` remote to the fork URL)

**Patch scope**: The fork adds premultiplied alpha support in `wgpu-hal/src/dx12/surface.rs` (swapchain creation path) and exposes `CompositeAlphaMode::PreMultiplied` in `wgpu-types/src/lib.rs`.

---

## Building

```bash
# Standard build (no test mode)
cargo build --release

# Test mode build (watermark + forced passthrough + TRACE logging)
cargo build --release --features test_mode

# Tracy profiler build
cargo build --release --features tracy
```

## Running

```bash
# Run the PoC binary
cargo run --release

# Run with test mode
cargo run --release --features test_mode

# Run the minimal example (bare-bones overlay without PoC features)
cargo run --release -p glass-poc --example minimal
```

## Examples

### Minimal Example

A minimal standalone example is provided at `glass-poc/examples/minimal.rs`. This demonstrates the essential bootstrap sequence using existing APIs only:

1. Tracing init
2. DPI awareness
3. Overlay window creation
4. DirectComposition initialization
5. wgpu Renderer initialization
6. Compositor commit
7. Initial render
8. Message loop

The minimal example **omits** PoC-specific features:
- Anti-cheat startup checks
- Config loading and hot-reload
- Module registry (clock, system stats, FPS counter)
- Layout system and widget positioning
- Demo interactive rectangles

This serves as a clean starting point for custom overlay implementations without the PoC harness overhead.

## Configuration

The overlay reads configuration from `config.ron` at startup. Changes are hot-reloaded at runtime via a file watcher (`notify` crate). See `glass-overlay/src/config.rs` for the `ConfigStore` API.

## Input Modes

- **Passive mode** (default): Overlay is click-through; all input passes to underlying windows.
- **Interactive mode**: Overlay captures input in hit-testable regions. Toggle via configured hotkey.
- **Test mode override**: When built with `--features test_mode`, interactive mode is forcibly disabled (overlay remains click-through always).

## License

MIT
