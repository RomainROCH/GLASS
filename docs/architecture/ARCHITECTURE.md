# GLASS Architecture

## Philosophy

GLASS is a **generic Windows overlay framework**. It creates a transparent,
always-on-top window backed by DirectComposition and renders through wgpu on
DX12. Applications build on GLASS by implementing `OverlayModule` and injecting
data through callbacks — GLASS never knows or cares where the data comes from.

### What GLASS is

- A reusable library for building transparent overlay UIs on Windows.
- An external-process overlay — always its own window, never injected into
  another process.
- A retained-mode scene graph with dirty tracking and zero-alloc steady state.
- A modular widget system where each module owns its scene nodes and lifecycle.

### What GLASS is not

- Not a cross-platform toolkit. It depends on DirectComposition + DX12. There
  is no platform abstraction layer (see [ADR: Windows-only](#adr-windows-only-is-a-feature-not-a-limitation)).
- Not a data bus. GLASS has no opinion on where data originates. Modules receive
  data through callback injection; the consumer application handles routing.
- Not a game hook. GLASS never injects DLLs, intercepts Present calls, or reads
  another process's memory. It is anti-cheat safe by design.

---

## High-Level Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                      Consumer Application                        │
│              (e.g. Pulse, or your own binary)                    │
│                                                                  │
│   ┌──────────┐  ┌──────────────┐  ┌────────────────────────┐    │
│   │ Data     │  │ IPC / ETW /  │  │  OverlayModule impls   │    │
│   │ Sources  │──│ Callbacks    │──│  (custom widgets)       │    │
│   └──────────┘  └──────────────┘  └───────────┬────────────┘    │
└───────────────────────────────────────────────┼──────────────────┘
                                                │ registers modules
┌───────────────────────────────────────────────┼──────────────────┐
│                        glass-overlay                             │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    Module Layer                              │ │
│  │  ModuleRegistry · LayoutManager · WidgetWrapper · Anchor    │ │
│  │  ClockModule · SystemStatsModule · FpsCounterModule          │ │
│  └──────────────────────────┬──────────────────────────────────┘ │
│                             │ scene graph ops                    │
│  ┌──────────────────────────▼──────────────────────────────────┐ │
│  │                  Scene Graph Layer                           │ │
│  │  Scene · NodeId · SceneNode (Rect | Text) · dirty tracking  │ │
│  └──────────────────────────┬──────────────────────────────────┘ │
│                             │ render submission                   │
│  ┌──────────────────────────▼──────────────────────────────────┐ │
│  │                    Render Layer                              │ │
│  │  wgpu DX12 · glyphon text · HDR detect + SDR fallback      │ │
│  │  PreMultiplied alpha · Mailbox present · LowPower GPU       │ │
│  └──────────────────────────┬──────────────────────────────────┘ │
│                             │ binds to visual                    │
│  ┌──────────────────────────▼──────────────────────────────────┐ │
│  │                 Composition Layer                            │ │
│  │  DComp Device → Target(HWND) → Visual → wgpu surface       │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────────┐    │
│  │  Input Layer  │  │  Config Layer │  │  Safety Layer     │    │
│  │  Passive /    │  │  RON / TOML   │  │  Anti-cheat       │    │
│  │  Interactive  │  │  hot-reload   │  │  detection         │    │
│  │  HitTester    │  │  ArcSwap      │  │  (feature-gated)  │    │
│  └───────────────┘  └───────────────┘  └───────────────────┘    │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                         glass-core                               │
│                   GlassError (shared error type)                 │
└──────────────────────────────────────────────────────────────────┘
```

---

## Workspace Crates

| Crate | Role | Key exports |
|---|---|---|
| `glass-core` | Shared core types | `GlassError` |
| `glass-overlay` | Reusable overlay library | `Compositor`, `Renderer`, `Scene`, `OverlayModule`, `LayoutManager`, `ConfigStore`, `InputManager` |
| `glass-starter` | Reference application + examples | `main.rs`, `examples/minimal.rs` |

---

## Architecture Layers

### 1. Composition Layer

**Source:** `compositor.rs` · **Deep dive:** [composition-pipeline.md](composition-pipeline.md)

Creates the DirectComposition device, target, and visual that make true per-pixel
alpha transparency possible. HWND-based DX12 swapchains only support
`AlphaMode::Opaque`; DirectComposition bypasses this by using
`CreateSwapChainForComposition` with `DXGI_ALPHA_MODE_PREMULTIPLIED`. The
`Compositor` struct owns the DComp lifetime and exposes a `visual_handle()` that
wgpu binds to via `SurfaceTargetUnsafe::CompositionVisual`.

### 2. Render Layer

**Source:** `renderer.rs`, `hdr.rs`, `text_renderer.rs` · **Deep dive:** [composition-pipeline.md](composition-pipeline.md)

Initializes a wgpu DX12 device with `PowerPreference::LowPower` (the overlay
must not steal GPU cycles from the game), creates a surface bound to the
DirectComposition visual, and runs a retained render loop. Text is rendered
through glyphon. HDR is detected via `IDXGIOutput6` with automatic SDR fallback
and a `--force-sdr` escape hatch. Presentation uses `PresentMode::Mailbox` for
low latency.

**Critical implementation detail — wgpu patches:** wgpu-hal 24.x hardcodes
`Opaque` alpha and `DXGI_ALPHA_MODE_IGNORE`. The `third_party/wgpu/` directory
contains patched forks of `wgpu-core`, `wgpu-hal`, `wgpu-types`, and `naga`.
Patching only wgpu-hal is **not** sufficient — wgpu-core must also be patched or
`alpha_modes` returns only `[Opaque]` and the background renders as solid black.

### 3. Scene Graph Layer

**Source:** `scene.rs` · **Deep dive:** [scene-graph.md](scene-graph.md)

A retained scene graph where nodes are created once and only re-uploaded to the
GPU when marked dirty. `Scene` owns a flat `Vec<NodeEntry>` with per-node dirty
flags and a global dirty flag. Node types are `Rect` (solid-colour rectangle) and
`Text` (glyphon-rendered text run). In steady state — when no module updates any
node — the render path produces zero heap allocations and zero GPU buffer uploads.

### 4. Module Layer

**Source:** `modules/mod.rs`, `modules/clock.rs`, `modules/system_stats.rs`, `modules/fps_counter.rs`, `layout.rs` · **Deep dive:** [module-system.md](module-system.md)

The `OverlayModule` trait defines the widget lifecycle: `info()` → `init(scene)`
→ `update(scene, dt)` → `deinit(scene)`. Modules own their scene node IDs and
manage their own cleanup. `ModuleRegistry` holds a flat `Vec<Box<dyn
OverlayModule>>` and drives batch init/update/deinit. `LayoutManager` positions
modules using `Anchor`-based screen-relative placement via `WidgetWrapper`, and
recalculates positions on resize. Data enters modules through callback injection
(e.g. `SystemStatsModule::set_temp_source()`), keeping GLASS sensor-agnostic.

### 5. Input Layer

**Source:** `input.rs`, `overlay_window.rs` · **Deep dive:** [input-system.md](input-system.md)

The overlay operates in two modes. **Passive** (default): `WS_EX_TRANSPARENT` is
set, making the window fully click-through — all mouse events pass to the
application below. **Interactive**: triggered by a global hotkey, the overlay
accepts mouse input on designated `InteractiveRect` regions for a configurable
timeout, then reverts to passive. `HitTester` provides Z-ordered rectangle
hit-testing. `InputManager` manages visual indicators (border + label) for the
interactive state.

### 6. Config Layer

**Source:** `config.rs` · **Deep dive:** [config-system.md](config-system.md)

`ConfigStore` loads an `OverlayConfig` from RON or TOML (detected by file
extension), stores it in an `ArcSwap` for lock-free reads from the render loop,
and watches the file with `notify` for hot-reload. When the file changes, the
new config is atomically swapped in. The application must still re-read and
reapply the snapshot — the store signals availability, not automatic application.

### 7. Safety Layer

**Source:** `safety.rs` · **Deep dive:** [safety-system.md](safety-system.md)

Feature-gated behind `gaming`. Scans for known anti-cheat systems using
exclusively passive, read-only Win32 APIs (`CreateToolhelp32Snapshot`,
`OpenSCManager` + `SERVICE_QUERY_STATUS`, `Path::exists`). Each detected
anti-cheat maps to a `DetectionPolicy`: **Block** (kernel-level AC like Vanguard
or Ricochet — refuse to start), **Warn** (user-mode AC like EAC or BattlEye —
start with a warning), or **Info** (VAC — log only). This layer runs before any
window or GPU initialization.

---

## Ecosystem Vision

GLASS is designed as the foundation layer of a multi-project ecosystem. Each
layer is an independent project with its own repository and release cycle.

```
┌───────────────────────────────────────────────────────┐
│  [Future]  Frame limiter                              │
│  Hook Present, scanline sync — separate concern       │
├───────────────────────────────────────────────────────┤
│  Pulse     Gaming OSD                                 │
│  FPS, CPU temp, GPU stats, frametime graph            │
│  Built on GLASS · injects data via callbacks          │
├───────────────────────────────────────────────────────┤
│  GLASS     Generic overlay framework                  │
│  Window · render · tray · scene graph · modules       │
└───────────────────────────────────────────────────────┘
```

**GLASS** is agnostic to data provenance. A safe application (Pulse, reading
system metrics via ETW) and an unsafe application (reading game memory via DLL
injection, forwarding over IPC to GLASS) can both build on GLASS without any
changes to the framework.

---

## Architectural Decisions

These decisions were made deliberately during design sessions. They are load-bearing
constraints, not defaults to revisit.

### ADR: Windows-only is a feature, not a limitation

The overlay depends on DirectComposition + DX12. macOS would need Core Animation
+ Metal; Linux Wayland forbids unmanaged overlays entirely. A platform
abstraction layer would be either too fine-grained to be useful or too
coarse-grained to preserve per-platform capabilities. 96%+ of target users (PC
gamers) are on Windows. This is a deliberate scoping decision.

### ADR: External process, no DLL injection

GLASS is always a separate window in its own process. It never injects into
another process, hook Present calls, or touch another process's memory. This
makes it inherently anti-cheat safe. Discord rebuilt their overlay in 2025 to
this exact external-process model for the same reasons.

### ADR: No DataBus in GLASS

Each module manages its own data source via callback injection. The consumer
application (e.g. Pulse) handles data routing. GLASS stays generic and simple —
no centralized message bus, no pub/sub, no event system.

### ADR: In-process plugins only

Modules are in-process implementations of the `OverlayModule` trait.
Out-of-process plugins are the consumer's problem — they can create an
`IpcBridgeModule` that receives data via named pipe and renders it. GLASS does
not need to know about IPC transports.

### ADR: wgpu over raw D3D12

wgpu is the right abstraction layer for a generic framework that needs to manage
its own swapchain lifecycle. Raw D3D12 only makes sense for in-process hooking
scenarios where you intercept an existing swapchain — which GLASS explicitly does
not do.

### ADR: Callback injection for data sources

`SystemStatsModule` receives a `TempSourceFn` callback via
`set_temp_source()`. This keeps GLASS sensor-agnostic. Consumer applications
inject closures that read from whatever source they control (HWiNFO shared
memory, LibreHardwareMonitor WMI, NVML, etc.).

The full ADR log is maintained in [decisions.md](decisions.md).

---

## Extension Points

GLASS is extended by consumer applications at these integration surfaces:

| Extension point | Mechanism | Example |
|---|---|---|
| **Custom modules** | Implement `OverlayModule` trait | A frametime-graph widget |
| **Data injection** | Callback closures on built-in modules | `SystemStatsModule::set_temp_source(cb)` |
| **IPC bridging** | Consumer-authored `IpcBridgeModule` | Named pipe receiving data from an injected DLL |
| **Config format** | Add fields to `OverlayConfig` (RON/TOML) | Custom per-module config sections |
| **Layout anchoring** | `WidgetWrapper` with `Anchor` variants | Pin a widget to `BottomRight` with margin |
| **Safety policy** | Enable `gaming` feature, extend `SystemProbe` | Add detection for a new anti-cheat |

### Minimal consumer integration

```rust
use glass_overlay::{
    Compositor, ConfigStore, LayoutManager, Renderer,
    WidgetWrapper, Anchor, SystemStatsModule,
};

// 1. Load config
let store = ConfigStore::load("config.ron")?;
store.watch()?;

// 2. Create overlay window (see overlay_window module)
let hwnd = create_overlay_window()?;

// 3. Set up composition + rendering
let compositor = Compositor::new(hwnd)?;
let mut renderer = Renderer::new(compositor.visual_handle(), hwnd)?;
compositor.commit()?;

// 4. Register modules with data callbacks
let mut stats = SystemStatsModule::new();
stats.set_temp_source(|| read_hwinfo_shared_memory());

// 5. Position via layout system
let mut layout = LayoutManager::new(width, height);
layout.add(WidgetWrapper::new(Box::new(stats), Anchor::TopLeft, 10.0, 10.0));
layout.init_all(renderer.scene_mut());

// 6. Run message loop: update modules, render, present
```

---

## Companion Architecture Documents

| Document | Covers |
|---|---|
| [composition-pipeline.md](composition-pipeline.md) | DirectComposition + wgpu binding, alpha mode patching, HDR pipeline |
| [scene-graph.md](scene-graph.md) | Retained scene graph, node types, dirty tracking, zero-alloc steady state |
| [module-system.md](module-system.md) | `OverlayModule` trait, `ModuleRegistry`, layout anchoring, callback injection |
| [input-system.md](input-system.md) | Passive/interactive mode switching, `HitTester`, hotkey, timeout |
| [config-system.md](config-system.md) | RON/TOML loading, `ArcSwap` hot-reload, `ConfigStore` API |
| [safety-system.md](safety-system.md) | Anti-cheat detection, `DetectionPolicy`, `SystemProbe` |
| [decisions.md](decisions.md) | Full ADR (Architecture Decision Record) log |

---

## What to Read Next

- **First time here?** Start with the [README](../../README.md), then
  [getting-started.md](../getting-started.md).
- **Building an app on GLASS?** Read [library-consumer.md](../library-consumer.md),
  then [module-system.md](module-system.md).
- **Writing a custom widget?** Read [module-authoring.md](../module-authoring.md),
  then [scene-graph.md](scene-graph.md).
- **Understanding the render pipeline?** Read
  [composition-pipeline.md](composition-pipeline.md).
- **Why was X decided?** Check [decisions.md](decisions.md).
