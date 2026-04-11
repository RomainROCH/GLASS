# Composition & Rendering Pipeline

This document describes how GLASS composites a transparent overlay onto the
Windows desktop using DirectComposition, renders through wgpu on DX12, and
handles HDR detection, text rendering, and surface lifecycle. It is the
authoritative reference for the composition and render layers.

**Source files:** `compositor.rs`, `renderer.rs`, `hdr.rs`, `text_renderer.rs`,
`overlay_window.rs`

**See also:** [ARCHITECTURE.md](ARCHITECTURE.md) · [scene-graph.md](scene-graph.md)

---

## Table of Contents

1. [DirectComposition Setup](#directcomposition-setup)
2. [The wgpu Patch — Critical Knowledge](#the-wgpu-patch--critical-knowledge)
3. [wgpu Renderer](#wgpu-renderer)
4. [HDR Detection](#hdr-detection)
5. [Text Rendering](#text-rendering)
6. [Render Loop Flow](#render-loop-flow)
7. [Overlay Window](#overlay-window)
8. [Complete Initialization Sequence](#complete-initialization-sequence)

---

## DirectComposition Setup

**Source:** `compositor.rs`

HWND-based DX12 swapchains only support `alpha_modes = [Opaque]`. This means
every pixel would be fully opaque — useless for an overlay. DirectComposition
bypasses this limitation: `CreateSwapChainForComposition` supports
`DXGI_ALPHA_MODE_PREMULTIPLIED`, giving real per-pixel alpha transparency.

### Flow

```
DCompDevice → Target(HWND) → Visual → wgpu binds swapchain to Visual
```

### `Compositor` struct

```rust
pub struct Compositor {
    device:  IDCompositionDevice,
    _target: IDCompositionTarget,  // prevent drop — must outlive the surface
    visual:  IDCompositionVisual,
}
```

The `Compositor` owns three DirectComposition COM objects:

| Field | COM Interface | Purpose |
|---|---|---|
| `device` | `IDCompositionDevice` | Factory for targets and visuals; owns the commit transaction |
| `_target` | `IDCompositionTarget` | Binds a DComp visual tree to an HWND; the underscore prefix signals "kept alive, not directly used" |
| `visual` | `IDCompositionVisual` | The composition visual that wgpu binds its swapchain to |

### Key methods

| Method | Signature | Purpose |
|---|---|---|
| `new(hwnd)` | `fn new(hwnd: HWND) -> Result<Self, GlassError>` | Creates device, target, and visual. Sets the visual as the target's root. |
| `visual_handle()` | `fn visual_handle(&self) -> NonNull<c_void>` | Returns the raw visual pointer for `wgpu::SurfaceTargetUnsafe::CompositionVisual`. |
| `commit()` | `fn commit(&self) -> Result<(), GlassError>` | Commits pending DComp changes. **Must** be called after wgpu configures the surface so the swapchain binding (`SetContent`) takes effect. |

### Why DirectComposition?

DirectComposition is the only supported path for transparent DX12 overlays on
Windows. The alternatives and why they fail:

| Approach | Problem |
|---|---|
| HWND-based `CreateSwapChainForHwnd` | DX12 only returns `alpha_modes = [Opaque]` |
| `WS_EX_LAYERED` with `UpdateLayeredWindow` | GDI-based; no GPU acceleration, no DX12 interop |
| `WS_EX_LAYERED` with per-pixel alpha bitmap | Same — GDI only, 60 FPS ceiling, high CPU cost |
| DirectComposition + `CreateSwapChainForComposition` | ✅ Supports `DXGI_ALPHA_MODE_PREMULTIPLIED` |

---

## The wgpu Patch — Critical Knowledge

> ⚠️ **This section documents a non-obvious, load-bearing implementation detail.
> Misunderstanding the patch scope will result in a solid black background
> instead of transparency.**

### Problem

wgpu-hal 24.0.4 hardcodes `Opaque` alpha and `DXGI_ALPHA_MODE_IGNORE` for all
DX12 surfaces. Even when the underlying DXGI swapchain supports premultiplied
alpha (as with DirectComposition), wgpu never exposes it.

### Solution

`third_party/wgpu/` contains patched forks of four wgpu crates that add
`PreMultiplied` alpha support for DirectComposition, SurfaceHandle, and
SwapChainPanel targets.

### Patched crates

| Crate | Why patched |
|---|---|
| `wgpu-hal` | DX12 backend: changes `DXGI_ALPHA_MODE_IGNORE` → conditional `DXGI_ALPHA_MODE_PREMULTIPLIED` for composition targets |
| `wgpu-core` | Capability reporting: exposes `PreMultiplied` in the `alpha_modes` list returned by `get_capabilities()` |
| `wgpu-types` | Shared types: ensures wgpu-hal and wgpu-core resolve the same `wgpu-types` crate (avoids duplicate type errors) |
| `naga` | Shader compiler: patched to stay in sync with the wgpu workspace version |

### Cargo.toml patch section

```toml
[patch.crates-io]
wgpu-core  = { path = "third_party/wgpu/wgpu-core" }
wgpu-hal   = { path = "third_party/wgpu/wgpu-hal" }
wgpu-types = { path = "third_party/wgpu/wgpu-types" }
naga       = { path = "third_party/wgpu/naga" }
```

### Critical bug to remember

> **Patching ONLY `wgpu-hal` is NOT enough.**
>
> `wgpu-core` must also be patched. Without the wgpu-core patch, the
> `get_capabilities()` call still returns `alpha_modes = [Opaque]` regardless
> of what the HAL reports. The surface configures with `Opaque` alpha, DComp
> treats every pixel as fully opaque, and the overlay background renders as
> **solid black** instead of transparent.
>
> This bug took significant time to diagnose because the HAL patch appeared
> correct in isolation — the symptom only manifested at the wgpu-core
> capability-reporting layer.
>
> Additionally, `wgpu-types` must be patched so that wgpu-hal (which resolves
> `wgpu-types` via its workspace) and wgpu-core use the **exact same**
> `wgpu-types` crate. Without this, Rust sees two different `CompositeAlphaMode`
> types and compilation fails with duplicate type errors.

---

## wgpu Renderer

**Source:** `renderer.rs`

### `Renderer` struct

```rust
pub struct Renderer {
    device:         wgpu::Device,
    queue:          wgpu::Queue,
    surface:        wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    color_pipeline: &'static str,
    scene:          Scene,
    text_engine:    TextEngine,
}
```

The `Renderer` is the central GPU owner. It holds the wgpu device, queue, and
surface, plus the retained [scene graph](scene-graph.md) and the glyphon-based
text engine. It is **not** `Send`/`Sync` — it must be used on the thread that
created it (the message-loop thread).

### Initialization sequence

`Renderer::new(visual: NonNull<c_void>, hwnd: HWND)` performs:

1. **Read client rect** — `GetClientRect(hwnd)` for initial surface dimensions,
   with `.max(1)` guards against zero-sized surfaces.
2. **Create DX12-only instance** — `wgpu::Instance` with `Backends::DX12`.
3. **Create surface** — `create_surface_unsafe(CompositionVisual(visual))`.
   The visual pointer must remain valid for the surface's lifetime.
4. **Request adapter** — `PowerPreference::LowPower`. The overlay must not
   steal GPU cycles from the foreground application.
5. **Request device** — default descriptor, label `"GLASS Device"`.
6. **Query capabilities** — `surface.get_capabilities(&adapter)` returns
   supported formats and alpha modes.
7. **Detect HDR** — calls `hdr::detect_primary_hdr()` and selects format.
8. **Select alpha mode** — `select_composition_alpha_mode()` requires
   `PreMultiplied` in the capabilities list; fails otherwise.
9. **Configure surface** — `PresentMode::Mailbox`,
   `desired_maximum_frame_latency: 1`, selected format and alpha mode.
10. **Create TextEngine** — glyphon setup with the selected format.

### `select_composition_alpha_mode()`

```rust
fn select_composition_alpha_mode(
    alpha_modes: &[wgpu::CompositeAlphaMode],
) -> Result<wgpu::CompositeAlphaMode, GlassError>
```

Returns `PreMultiplied` if present in the list; returns
`GlassError::WgpuInit` otherwise. This is the hard gate that catches a missing
wgpu patch — if the patch is incomplete, `alpha_modes` will be `[Opaque]` and
initialization fails with a clear error message.

### Surface configuration

| Parameter | Value | Rationale |
|---|---|---|
| `usage` | `RENDER_ATTACHMENT` | Standard render target usage |
| `format` | `Rgba16Float` (HDR) or `Bgra8UnormSrgb` (SDR) | See [HDR Detection](#hdr-detection) |
| `present_mode` | `Mailbox` | Low-latency; drops stale frames rather than queuing |
| `desired_maximum_frame_latency` | `1` | Minimize input-to-photon latency |
| `alpha_mode` | `PreMultiplied` | Required for transparent DComp composition |
| `view_formats` | `[]` | No additional view formats needed |

### Surface error recovery

| Error | Action |
|---|---|
| `Lost` or `Outdated` | Reconfigure the surface and retry acquire once. If retry fails, return `GlassError`. |
| `Timeout` | Skip this frame (transient). |
| Other | Return `GlassError` (fatal). |

---

## HDR Detection

**Source:** `hdr.rs`

### Detection mechanism

Uses `IDXGIOutput6::GetDesc1()` to query the primary display's color space.

```rust
pub enum DisplayCapability {
    Hdr,     // scRGB or ST.2084
    Sdr,     // explicit SDR
    Unknown, // detection failed
}
```

### Color space mapping

| DXGI Color Space | `DisplayCapability` |
|---|---|
| `DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709` (scRGB) | `Hdr` |
| `DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020` (ST.2084) | `Hdr` |
| Any other | `Sdr` |

### Fallback conditions

The SDR pipeline is used when:

- `IDXGIOutput6` is not available (Windows pre-1803)
- `GetDesc1()` or any DXGI enumeration call fails
- The `--force-sdr` command-line flag is set

### Format selection

```rust
pub fn choose_surface_format(
    capabilities: &[wgpu::TextureFormat],
    hdr: DisplayCapability,
    force_sdr: bool,
) -> (wgpu::TextureFormat, &'static str)
```

| Pipeline | Texture Format | Condition |
|---|---|---|
| HDR/scRGB | `Rgba16Float` | HDR detected, not force-SDR, format supported |
| SDR/sRGB | `Bgra8UnormSrgb` | Everything else (preferred SDR format) |
| SDR/fallback | `capabilities[0]` | `Bgra8UnormSrgb` not in capabilities |

---

## Text Rendering

**Source:** `text_renderer.rs`

### `TextEngine` struct

```rust
pub struct TextEngine {
    font_system:   FontSystem,
    swash_cache:   SwashCache,
    atlas:         TextAtlas,
    viewport:      Viewport,
    text_renderer: TextRenderer,
    buffer_pool:   Vec<Buffer>,
}
```

`TextEngine` wraps the full glyphon stack: `FontSystem` (cosmic-text font
loading), `SwashCache` (glyph rasterization cache), `TextAtlas` (GPU texture
atlas), `Viewport` (resolution tracking), and `TextRenderer` (draw command
generation).

### Buffer pool

`buffer_pool: Vec<Buffer>` is a pre-allocated pool of cosmic-text buffers that
grows as needed but is **reused across frames** — no heap allocations in steady
state. Each text node in the scene maps to one buffer in the pool.

### Per-frame flow

1. **`prepare(device, queue, scene, width, height)`**:
   - Updates viewport resolution.
   - Collects all `SceneNode::Text` nodes from the scene.
   - Grows the buffer pool if more text nodes exist than buffers.
   - Sets metrics, size, and text content on each buffer.
   - Shapes text via cosmic-text.
   - Builds `TextArea` list with position, bounds, and color.
   - Calls `text_renderer.prepare()` to upload glyphs to the atlas.

2. **`render(render_pass)`**:
   - Calls `text_renderer.render()` — draws all prepared text in a single pass.

### Color conversion

Scene `Color { r, g, b, a: f32 }` is converted to glyphon `Color::rgba(u8,
u8, u8, u8)` by multiplying each channel by 255. Colors are expected to be in
premultiplied alpha space.

---

## Render Loop Flow

Each frame executes these steps in order:

```
1. text_engine.prepare()     ← collect Text nodes, layout, upload glyphs
2. surface.get_current_texture()  ← acquire next swapchain image
   └─ on Lost/Outdated: reconfigure + retry once
   └─ on Timeout: skip frame, return Ok
3. device.create_command_encoder()
4. encoder.begin_render_pass()
   └─ clear color: rgba(0, 0, 0, 0)  ← fully transparent
5. text_engine.render()      ← draw all text into the render pass
6. queue.submit() + frame.present()
7. scene.clear_dirty()       ← reset all dirty flags
```

The clear color `rgba(0, 0, 0, 0)` is critical. In premultiplied alpha space,
`(0, 0, 0, 0)` means fully transparent. Any other clear color would make the
overlay background partially or fully visible.

---

## Overlay Window

**Source:** `overlay_window.rs`

### Extended window styles

```rust
let ex_style = WS_EX_TOPMOST
    | WS_EX_TOOLWINDOW
    | WS_EX_NOACTIVATE
    | WS_EX_TRANSPARENT
    | WS_EX_LAYERED
    | WS_EX_NOREDIRECTIONBITMAP;
```

| Style | Purpose |
|---|---|
| `WS_EX_TOPMOST` | Always on top of other windows |
| `WS_EX_TOOLWINDOW` | Hidden from Alt+Tab and taskbar |
| `WS_EX_NOACTIVATE` | Never steals focus from the foreground application |
| `WS_EX_TRANSPARENT` | Click-through in passive mode (mouse events pass to windows below) |
| `WS_EX_LAYERED` | Enables per-pixel alpha transparency at the Win32 level |
| `WS_EX_NOREDIRECTIONBITMAP` | Suppresses the GDI redirection surface; all content comes from DComp |

### Window characteristics

- **Style:** `WS_POPUP` (no title bar, no borders)
- **Dimensions:** fullscreen (`GetSystemMetrics(SM_CXSCREEN)` × `SM_CYSCREEN`)
- **Created without `WS_VISIBLE`** to avoid a flash of opaque GDI surface before
  DirectComposition takes over
- **System tray icon** for clean exit (right-click → Quit)

### DPI awareness

```rust
pub fn set_dpi_awareness() {
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
}
```

**Must** be called before any window creation. `PerMonitorAwareV2` ensures:
- Correct sizing on high-DPI displays
- Correct `GetClientRect` values for surface configuration
- No DPI virtualization that would blur the overlay

---

## Complete Initialization Sequence

```
Application startup
│
├─ 1. set_dpi_awareness()
│     └─ SetProcessDpiAwarenessContext(PerMonitorAwareV2)
│        Must be BEFORE any HWND creation
│
├─ 2. ConfigStore::load("config.ron")
│     └─ Parse config, start file watcher
│
├─ 3. create_overlay_window(timeout, hotkey_vk, hotkey_mods, app_name)
│     ├─ RegisterClassExW(GLASS_OVERLAY)
│     ├─ GetSystemMetrics → fullscreen dimensions
│     ├─ CreateWindowExW(EX_TOPMOST | EX_TOOLWINDOW | EX_NOACTIVATE
│     │                  | EX_TRANSPARENT | EX_LAYERED
│     │                  | EX_NOREDIRECTIONBITMAP,
│     │                  WS_POPUP, fullscreen)
│     ├─ Add system tray icon
│     └─ Register global hotkey
│
├─ 4. Compositor::new(hwnd)
│     ├─ DCompositionCreateDevice(None) → IDCompositionDevice
│     ├─ device.CreateTargetForHwnd(hwnd) → IDCompositionTarget
│     ├─ device.CreateVisual() → IDCompositionVisual
│     └─ target.SetRoot(visual)
│
├─ 5. Renderer::new(compositor.visual_handle(), hwnd)
│     ├─ GetClientRect(hwnd) → (width, height)
│     ├─ wgpu::Instance::new(DX12)
│     ├─ instance.create_surface_unsafe(CompositionVisual(ptr))
│     ├─ instance.request_adapter(LowPower)
│     ├─ adapter.request_device("GLASS Device")
│     ├─ surface.get_capabilities(adapter)
│     │   └─ alpha_modes must contain PreMultiplied (patch gate)
│     ├─ hdr::detect_primary_hdr()
│     │   └─ IDXGIOutput6::GetDesc1() → HDR or SDR
│     ├─ hdr::choose_surface_format() → Rgba16Float or Bgra8UnormSrgb
│     ├─ select_composition_alpha_mode() → PreMultiplied
│     ├─ surface.configure(Mailbox, latency=1, PreMultiplied)
│     └─ TextEngine::new(device, queue, format)
│
├─ 6. compositor.commit()
│     └─ IDCompositionDevice::Commit()
│        Finalizes swapchain binding — MUST happen after surface.configure
│
├─ 7. LayoutManager + module registration
│     ├─ Register OverlayModule impls
│     ├─ layout.init_all(scene) → modules add nodes
│     └─ ShowWindow(hwnd, SW_SHOWNOACTIVATE)
│
└─ 8. run_message_loop()
      ├─ SetTimer(MODULE_UPDATE_TIMER_ID)
      └─ loop {
           GetMessageW → TranslateMessage → DispatchMessageW
           on WM_TIMER:
             modules.update_all(scene, dt)
             if scene.is_dirty() → renderer.render()
         }
```

---

## Related Documents

- [ARCHITECTURE.md](ARCHITECTURE.md) — high-level system overview
- [scene-graph.md](scene-graph.md) — retained scene graph, node types, dirty tracking
- [decisions.md](decisions.md) — full ADR log including wgpu patch rationale
