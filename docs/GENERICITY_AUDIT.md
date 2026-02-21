# Genericity Audit Report

## 1. Glass-Overlay Modules

This section audits all core modules under `glass-overlay/src` for platform-specific, wgpu-fork-specific, and PoC-specific dependencies.

---

### compositor.rs

**File**: `glass-overlay/src/compositor.rs`

**Windows-specific**:
- Lines 1-75: Entire module — DirectComposition is a Windows-only API
- Line 13: `use windows::core::Interface` — Windows SDK binding
- Line 14: `use windows::Win32::Foundation::HWND` — Windows handle type
- Line 15: `use windows::Win32::Graphics::DirectComposition::*` — DComp APIs
- Lines 22-24: `IDCompositionDevice`, `IDCompositionTarget`, `IDCompositionVisual` — COM interface types
- Lines 34-48: `DCompositionCreateDevice`, `CreateTargetForHwnd`, `CreateVisual`, `SetRoot` — Win32 API calls
- Line 62: Returns `NonNull<c_void>` pointer to `IDCompositionVisual` for wgpu surface binding

**wgpu-fork-specific**:
- Lines 4-6: Comment references `CreateSwapChainForComposition` + `DXGI_ALPHA_MODE_PREMULTIPLIED` — rationale for DirectComposition usage; core to premultiplied-alpha transparency
- Line 60: Comment mentions `wgpu::SurfaceTargetUnsafe::CompositionVisual` enum variant (exists in forked wgpu)

**PoC-specific**: None (core transparency mechanism)

**Generalization path**: Replace with platform abstraction (Linux/macOS: separate implementations; compositor trait with Win32/X11/Wayland/Quartz backends)

---

### renderer.rs

**File**: `glass-overlay/src/renderer.rs`

**Windows-specific**:
- Line 18: `use windows::Win32::Foundation::HWND` — window handle type
- Line 19: `use windows::Win32::UI::WindowsAndMessaging::GetClientRect` — Win32 sizing API
- Line 120: `hwnd: HWND` parameter
- Lines 128-135: `GetClientRect(hwnd, &mut rect)` — Win32 call to query window dimensions

**wgpu-fork-specific**:
- Line 139: `wgpu::Instance::new` with `backends: wgpu::Backends::DX12` — hardcoded to DX12 backend
- Lines 145-150: `wgpu::SurfaceTargetUnsafe::CompositionVisual(visual.as_ptr())` — forked wgpu API for DirectComposition binding
- Lines 187-193: `wgpu::CompositeAlphaMode::PreMultiplied` selection — relies on fork's premultiplied-alpha support for composition surfaces

**PoC-specific**:
- Lines 24-93: `SHADER_SRC` const — hardcoded WGSL triangle shader (PoC demo geometry)
- Lines 54-93: Conditional `#[cfg(feature = "test_mode")]` shader variant with watermark rectangles
- Lines 268-285: Test-mode watermark text nodes added to scene (PoC validation feature)
- Lines 406-411: Hardcoded `rpass.draw(0..vertex_count, 0..1)` with 3 or 9 vertices for PoC triangle

**Generalization path**:
- Replace HWND with cross-platform window handle abstraction (winit `Window`)
- Parameterize wgpu backend selection (Vulkan/Metal/DX12/GL)
- Remove hardcoded triangle shader; modularize geometry pipeline

---

### overlay_window.rs

**File**: `glass-overlay/src/overlay_window.rs`

**Windows-specific**:
- Lines 1-552: Entire module — Win32 HWND, message loop, system tray, DPI awareness
- Lines 24-32: `use windows::Win32::*` — all imports are Win32 APIs
- Lines 40-52: `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` — Windows 10 DPI API
- Lines 58-68: `MessageBoxW` — modal error dialog (Win32)
- Lines 88-147: `CreateWindowExW` with `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOREDIRECTIONBITMAP` — Windows extended styles for click-through overlay
- Line 140: `SetLayeredWindowAttributes` — layered window alpha control
- Line 151: `SetWindowLongPtrW(hwnd, GWLP_USERDATA, ...)` — Win32 window data storage
- Lines 193-220: `Shell_NotifyIconW` — system tray icon (Windows shell API)
- Lines 308-423: `wnd_proc` — Win32 window procedure handling `WM_NCHITTEST`, `WM_HOTKEY`, `WM_TIMER`, etc.
- Lines 446-530: `GetMessageW` + `DispatchMessageW` — Win32 message loop

**wgpu-fork-specific**: None (window management layer)

**PoC-specific**:
- Lines 142-147: `test_mode::TITLE_PREFIX` — prepends "[MODE TEST]" to window title in PoC builds
- Lines 154-175: Test mode skips hotkey registration if `FORCE_INPUT_PASSTHROUGH` is set
- Lines 207-213: System tray tooltip uses `test_mode::TRAY_TOOLTIP` (PoC labeling)

**Generalization path**: Replace with cross-platform windowing library (winit), abstract message loop and DPI handling

---

### input.rs

**File**: `glass-overlay/src/input.rs`

**Windows-specific**:
- Lines 17-26: Custom Windows messages `WM_GLASS_MODE_INTERACTIVE` / `WM_GLASS_MODE_PASSIVE` (`WM_APP + 10/11`)
- Line 23: `INTERACTIVE_TIMER_ID: usize = 42` — Win32 timer ID
- Line 25: `HOTKEY_ID: i32 = 1` — Win32 hotkey ID

**wgpu-fork-specific**: None (input layer)

**PoC-specific**: None (core interactive mode system)

**Generalization path**: Abstract timer/hotkey system for cross-platform input handling

---

### config.rs

**File**: `glass-overlay/src/config.rs`

**Windows-specific**:
- Lines 90-97: `hotkey_vk: u32` + `hotkey_modifiers: u32` — Win32 virtual key codes and `MOD_*` flags
- Line 107: Default `0x7B` (VK_F12) — Windows-specific key code

**wgpu-fork-specific**: None (configuration layer)

**PoC-specific**: None (generic config infrastructure)

**Generalization path**: Replace VK codes with cross-platform key enum (winit `VirtualKeyCode`)

---

### modules/*

**File**: `glass-overlay/src/modules/mod.rs`, `clock.rs`, `fps_counter.rs`, `system_stats.rs`

**Windows-specific**: None (uses cross-platform APIs: `chrono` for clock, `sysinfo` for system stats)

**wgpu-fork-specific**: None (scene-graph layer only)

**PoC-specific**: None (generic HUD module system)

**Generalization path**: Already generic

---

### layout.rs

**File**: `glass-overlay/src/layout.rs`

**Windows-specific**: None (generic anchor-based positioning)

**wgpu-fork-specific**: None (layout math only)

**PoC-specific**: None (widget layout system)

**Generalization path**: Already generic

---

### hdr.rs

**File**: `glass-overlay/src/hdr.rs`

**Windows-specific**:
- Lines 1-132: Entire module — uses `IDXGIFactory1`, `IDXGIOutput6` for HDR detection
- Lines 8-10: `use windows::Win32::Graphics::Dxgi::*` — Windows DXGI APIs
- Lines 37-72: `CreateDXGIFactory1`, `EnumAdapters1`, `EnumOutputs`, `cast::<IDXGIOutput6>` — DXGI interface queries
- Lines 94-95: `DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020` / `DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709` — Windows HDR color space constants

**wgpu-fork-specific**:
- Lines 134-166: `choose_surface_format` selects `wgpu::TextureFormat::Rgba16Float` for HDR or `Bgra8UnormSrgb` for SDR — depends on wgpu's format support

**PoC-specific**:
- Line 180: `--force-sdr` command-line flag check (PoC override)

**Generalization path**: Abstract HDR detection per platform (Windows: DXGI, Linux: Wayland HDR protocols or X11 extensions, macOS: Core Graphics)

---

### safety.rs

**File**: `glass-overlay/src/safety.rs`

**Windows-specific**:
- Lines 192-217: `win32_service_exists` — `OpenSCManagerW` + `OpenServiceW` (Windows Services API)
- Lines 223-264: `win32_process_running` — `CreateToolhelp32Snapshot` + `Process32FirstW/NextW` (Windows process enumeration)
- Line 179: `Path::new("C:\\Windows\\System32\\drivers\\{name}")` — Windows driver path

**wgpu-fork-specific**: None (anti-cheat detection layer)

**PoC-specific**:
- Lines 120-156: `AC_SIGNATURES` data — detects Vanguard, Ricochet, EAC, BattlEye, VAC (research-specific safety checks)

**Generalization path**: Replace with cross-platform process/service query (Linux: `/proc`, macOS: `libproc`, or stub out on non-Windows)

---

### scene.rs

**File**: `glass-overlay/src/scene.rs`

**Windows-specific**: None (pure scene graph data structure)

**wgpu-fork-specific**: None (retained rendering state)

**PoC-specific**: None (generic scene graph)

**Generalization path**: Already generic

---

### text_renderer.rs

**File**: `glass-overlay/src/text_renderer.rs`

**Windows-specific**: None (uses glyphon crate, which is cross-platform)

**wgpu-fork-specific**:
- Lines 46-66: `TextAtlas::with_color_mode`, `TextRenderer::new` — depends on wgpu device/queue; format-agnostic but uses wgpu primitives

**PoC-specific**: None (generic text engine)

**Generalization path**: Already generic (glyphon is cross-platform)

---

### test_mode.rs

**File**: `glass-overlay/src/test_mode.rs`

**Windows-specific**: None (feature-flag constants only)

**wgpu-fork-specific**: None

**PoC-specific**:
- Lines 1-50: Entire module — defines PoC watermark text, forced passthrough mode, window title prefix for test builds
- Lines 22-26: `WATERMARK_LINES` — French research prototype notice
- Line 35: `FORCE_INPUT_PASSTHROUGH: bool = true` — disables interactive hotkeys in test mode

**Generalization path**: Remove or stub out for production (PoC validation feature only)

---

### diagnostics.rs

**File**: `glass-overlay/src/diagnostics.rs`

**Windows-specific**:
- Lines 112-127: `OsInfo::capture` — `GetVersionExW` (deprecated Win32 version API)
- Lines 131-158: `GpuInfo::capture` — `CreateDXGIFactory1`, `EnumAdapters1`, `GetDesc1` (DXGI adapter info)
- Lines 161-169: `DwmInfo::capture` — `DwmIsCompositionEnabled` (DWM state query)
- Lines 173-227: `OutputInfo::enumerate` — DXGI output enumeration + HDR detection via `IDXGIOutput6`
- Lines 231-254: `ProcessInfo::capture` — `GetWindowDpiAwarenessContext`, `GetAwarenessFromDpiAwarenessContext`, `GetDpiForWindow` (Windows DPI APIs)

**wgpu-fork-specific**: None (diagnostics capture layer)

**PoC-specific**:
- Lines 1-265: Entire module — GPU diagnostics dump for research/debugging (captures error context and color pipeline name for triage)

**Generalization path**: Abstract diagnostics per platform (Linux: `/sys`, `/proc`; macOS: system_profiler or ioreg)

---

## Summary: glass-overlay modules

| Module              | Windows-specific | wgpu-fork-specific | PoC-specific | Generalization Effort |
|---------------------|------------------|--------------------|--------------|-----------------------|
| `compositor.rs`     | ✅ (entire)      | ✅ (API usage)     | ❌           | High (requires platform abstraction) |
| `renderer.rs`       | ✅ (HWND, GetClientRect) | ✅ (DX12, CompositionVisual, PreMultiplied alpha) | ✅ (triangle shader, test watermark) | Medium (abstract window handle, remove PoC geometry) |
| `overlay_window.rs` | ✅ (entire)      | ❌                 | ✅ (test mode labels) | High (replace with winit or similar) |
| `input.rs`          | ✅ (WM_APP msgs, timer/hotkey IDs) | ❌                 | ❌           | Low (abstract timer/hotkey) |
| `config.rs`         | ✅ (VK codes)    | ❌                 | ❌           | Low (use cross-platform key enum) |
| `modules/*`         | ❌               | ❌                 | ❌           | None (already generic) |
| `layout.rs`         | ❌               | ❌                 | ❌           | None (already generic) |
| `hdr.rs`            | ✅ (DXGI HDR detection) | ✅ (format selection) | ✅ (--force-sdr flag) | Medium (abstract HDR detection) |
| `safety.rs`         | ✅ (entire)      | ❌                 | ✅ (AC signatures) | Medium (cross-platform process query or stub) |
| `scene.rs`          | ❌               | ❌                 | ❌           | None (already generic) |
| `text_renderer.rs`  | ❌               | ✅ (wgpu types)    | ❌           | None (glyphon is cross-platform) |
| `test_mode.rs`      | ❌               | ❌                 | ✅ (entire)  | Trivial (remove or stub out) |
| `diagnostics.rs`    | ✅ (entire)      | ❌                 | ✅ (entire)  | Medium (abstract diagnostics per platform) |

---

## Evidence Summary

**Highest-impact Windows dependencies**:
1. **`compositor.rs`** (DirectComposition) — premultiplied-alpha transparency mechanism
2. **`overlay_window.rs`** (Win32 HWND, layered windows, message loop) — window management
3. **`hdr.rs`** (DXGI) — HDR display detection

**Highest-impact wgpu-fork dependencies**:
1. **`renderer.rs`** lines 145-150 — `SurfaceTargetUnsafe::CompositionVisual` enum variant
2. **`renderer.rs`** lines 187-193 — `CompositeAlphaMode::PreMultiplied` support for composition surfaces

**PoC-specific code (safe to remove for production)**:
1. **`test_mode.rs`** — entire module (watermark, forced passthrough)
2. **`renderer.rs`** lines 24-93, 268-285, 406-411 — hardcoded triangle shader + test watermark
3. **`diagnostics.rs`** — entire module (GPU error dump for research)
4. **`safety.rs`** — AC detection (research-specific safety checks)

---

## Generalization Paths

### Path 1: Cross-Platform Window/Graphics (High Effort)
- Replace `overlay_window.rs` with **winit** (cross-platform windowing)
- Replace `compositor.rs` with per-platform transparency:
  - Windows: DirectComposition (keep existing)
  - Linux/X11: `_NET_WM_WINDOW_OPACITY` + shaped windows
  - Linux/Wayland: `zwlr_layer_shell_v1` or `xdg_toplevel` with alpha
  - macOS: `NSWindow` with `alphaValue` + `opaque = false`
- Abstract wgpu backend selection (Vulkan/Metal/DX12)
- Abstract HDR detection per platform

### Path 2: Minimal Standalone (Low Effort)
- Remove test_mode.rs, diagnostics.rs, safety.rs (PoC-only code)
- Replace hardcoded triangle shader with blank/user-provided geometry
- Keep Windows/DX12-only initially; document platform constraints
- Provide trait-based extensibility for future ports

### Path 3: Embedded Library (Medium Effort)
- Extract `scene.rs`, `text_renderer.rs`, `layout.rs`, `modules/*` into `glass-core`
- Expose platform-agnostic scene-graph API
- Let integrators provide their own window/compositor/backend bindings
- Document "bring your own window" integration guide

**Recommended**: **Path 2** for initial standalone release, then **Path 1** incrementally for cross-platform support.

---

## 2. Glass-PoC and Support Files

This section audits the PoC harness binary (`glass-poc/src/main.rs`, `alloc_tracker.rs`) and repo-level support files (workspace Cargo.toml, config, tooling scripts) for PoC-specific, workflow-specific, and generic concerns.

---

### main.rs

**File**: `glass-poc/src/main.rs`

**Windows-specific**:
- Line 28: `use glass_overlay::overlay_window` — imports Win32 window creation module
- Lines 76, 97: `overlay_window::show_error_dialog` — Win32 MessageBoxW modal dialogs
- Line 114: `overlay_window::set_dpi_awareness()` — Win32 DPI API
- Lines 143-147: `overlay_window::create_overlay_window(...)` — creates layered HWND with hotkey registration
- Lines 159-166: `Compositor::new(hwnd)` — DirectComposition initialization from HWND
- Lines 171-178: `Renderer::new(dcomp.visual_handle(), hwnd)` — binds wgpu DX12 renderer to DComp visual + HWND
- Lines 229-234: `overlay_window::get_hwnd_input_state(hwnd)` — Win32 window data storage access

**wgpu-fork-specific**:
- Line 171: `Renderer::new(dcomp.visual_handle(), hwnd)` — renderer uses forked wgpu APIs (`CompositionVisual`, `PreMultiplied` alpha)

**PoC-specific**:
- Lines 18: `mod alloc_tracker` — debug allocation profiling (PoC validation)
- Lines 35-38: Feature-gated `test_mode` vs `info` default tracing filter
- Lines 40-64: Tracy profiling integration (`#[cfg(feature = "tracy")]`)
- Lines 66-68: `alloc_tracker::install()` — debug-only allocation tracking
- Lines 82-111: Anti-cheat self-check (`AntiCheatDetector::scan()`) — passive kernel-AC detection before init; blocks startup if kernel AC detected (research safety feature)
- Lines 228-234: Demo interactive rect at (100, 100) 200×60 — PoC hit-testing example

**Generalization path**: 
- Remove `alloc_tracker`, Tracy integration, anti-cheat scan, demo interactive rect (PoC-only features)
- Replace `overlay_window` calls with cross-platform window library (e.g., winit)
- Keep retained-rendering + module-tick architecture (generic)

---

### alloc_tracker.rs

**File**: `glass-poc/src/alloc_tracker.rs`

**Windows-specific**: None (uses standard Rust alloc API)

**wgpu-fork-specific**: None (allocation profiling layer)

**PoC-specific**:
- Lines 1-80: Entire module — debug-mode per-frame allocation counter (PoC validation; warns if steady-state frames trigger heap allocations)
- Lines 45-47: `#[global_allocator]` static override (PoC diagnostics)
- Lines 10-13: `ALLOC_COUNT`, `INSTALLED` atomics for frame profiling

**Generalization path**: Remove entirely (PoC validation feature)

---

### Cargo.toml (workspace root)

**File**: `Cargo.toml`

**Windows-specific**:
- Lines 22-40: `windows` crate dependency with extensive Win32 feature flags (DirectComposition, DXGI, DWM, HiDpi, WindowsAndMessaging, Shell, ToolHelp, Services)
- Line 19: `wgpu` with `features = ["dx12"]` — hardcoded DX12 backend

**wgpu-fork-specific**:
- Lines 74-85: `[patch.crates-io]` section — overrides wgpu-hal, wgpu-types, naga with local fork in `third_party/wgpu`
- Lines 75-78: Comment explains fork rationale: "wgpu-hal 24.0.4 hardcodes Opaque alpha and DXGI_ALPHA_MODE_IGNORE. Our fork adds PreMultiplied support for DirectComposition."

**PoC-specific**:
- Lines 3-7: Workspace members: `glass-core`, `glass-overlay`, `glass-poc`
- Lines 8-10: `exclude = ["third_party/wgpu"]` — subtree directory
- Lines 13-14: Workspace metadata: `edition = "2024"`, `rust-version = "1.85"` (requires nightly Rust 2024 edition)

**Workflow-specific**:
- Lines 46-48: `tracing-tracy` dependency — Tracy profiler integration for performance analysis
- Lines 51-52, 63: `pollster`, `notify` dependencies — async blocking + config hot-reload
- Lines 43: `raw-window-handle` — window-handle interop (generic cross-platform trait)

**Generalization path**:
- Replace `dx12` with feature flags for multi-backend (vulkan, metal, dx12)
- Add Linux/macOS-specific dependencies conditionally (`#[cfg(target_os = "linux")]`)
- Retain fork until wgpu upstream merges premultiplied-alpha support, then revert to crates.io

---

### config.ron

**File**: `config.ron`

**Windows-specific**:
- Lines 15-16: `hotkey_vk: 0x7B` (F12), `hotkey_modifiers: 0` — Win32 virtual key code (VK_F12) + MOD_* flags

**wgpu-fork-specific**: None (config data only)

**PoC-specific**:
- Lines 1-20: Entire file — PoC overlay configuration (position, size, opacity, colors, hotkey, timeout)
- Lines 1-4: Comment mentions hot-reload + supported formats (.ron, .toml)

**Generalization path**: 
- Replace VK codes with cross-platform key enum (winit `VirtualKeyCode`)
- Keep position/size/opacity/colors (generic widget config)

---

### clippy.toml

**File**: `clippy.toml`

**Windows-specific**: None

**wgpu-fork-specific**: None

**PoC-specific**: None

**Workflow-specific**:
- Line 1: `msrv = "1.85"` — enforces minimum supported Rust version in Clippy lints

**Generalization path**: Already generic (standard Clippy config)

---

### tasks.sh

**File**: `tasks.sh`

**Windows-specific**: None (bash script; cross-platform workflow)

**wgpu-fork-specific**: None (invokes sync_wgpu.py)

**PoC-specific**: None

**Workflow-specific**:
- Lines 1-21: Entire file — task runner for wgpu subtree sync workflow
- Lines 5-15: Commands: `sync-status`, `sync-pull`, `sync-push`, `sync-init` — delegate to `sync_wgpu.py`
- Line 15: `uv sync` — initializes Python virtual environment (requires `uv` tool)

**Generalization path**: Already generic (standard git subtree workflow)

---

### sync_wgpu.py

**File**: `sync_wgpu.py`

**Windows-specific**: None (git subtree operations; cross-platform)

**wgpu-fork-specific**:
- Lines 1-253: Entire file — manages git subtree for `third_party/wgpu` fork
- Lines 27-32: Configuration constants: `WGPU_FORK_URL`, `WGPU_REMOTE`, `SUBTREE_PREFIX = "third_party/wgpu"`, `DEFAULT_BRANCH = "v24"` (tracks v24 series with premultiplied-alpha patches)
- Lines 96-113: `cmd_status` — shows subtree merge history for `third_party/wgpu`
- Lines 124-185: `cmd_setup` — adds wgpu fork remote + performs subtree add/adopt
- Lines 187-203: `cmd_pull` — fetches upstream wgpu changes (`--squash` merge)
- Lines 206-219: `cmd_push` — pushes local wgpu-hal patches back to fork

**PoC-specific**: None (fork maintenance tool)

**Workflow-specific**:
- Lines 1-253: Entire file — git subtree automation script (repository workflow)
- Lines 22-24: Imports: `argparse`, `subprocess`, `sys`, `pathlib` — standard Python CLI tooling
- Lines 35-56: `run()`, `git()` helpers — subprocess wrappers with logging
- Lines 72-79: `ensure_clean_worktree()` — abort if uncommitted changes present

**Generalization path**: 
- Keep for fork maintenance until wgpu upstream merges premultiplied-alpha support
- Remove script + subtree once upstream wgpu can be used directly from crates.io

---

## Summary: glass-poc and support files

| File                  | Windows-specific | wgpu-fork-specific | PoC-specific | Workflow-specific | Generalization Effort |
|-----------------------|------------------|--------------------|--------------|--------------------|----------------------|
| `main.rs`             | ✅ (overlay_window, DComp, HWND) | ✅ (renderer fork API) | ✅ (alloc_tracker, Tracy, AC scan, demo rect) | ❌ | Medium (remove PoC features, abstract window API) |
| `alloc_tracker.rs`    | ❌               | ❌                 | ✅ (entire)  | ❌                 | Trivial (delete file) |
| `Cargo.toml`          | ✅ (windows crate, dx12) | ✅ (patch.crates-io fork) | ✅ (workspace structure, edition 2024) | ✅ (tracy, notify) | Low (add multi-backend features, conditionally include deps) |
| `config.ron`          | ✅ (VK codes)    | ❌                 | ✅ (entire)  | ❌                 | Low (replace VK codes with cross-platform keys) |
| `clippy.toml`         | ❌               | ❌                 | ❌           | ✅ (MSRV)          | None (already generic) |
| `tasks.sh`            | ❌               | ❌                 | ❌           | ✅ (entire)        | None (already generic) |
| `sync_wgpu.py`        | ❌               | ✅ (entire)        | ❌           | ✅ (entire)        | None (keep until upstream merge) |

---

## Evidence Summary (PoC + Support)

**Highest-impact Windows dependencies (PoC harness)**:
1. **`main.rs`** lines 28, 76, 97, 114, 143-178 — Win32 overlay_window, DComp, HWND lifecycle
2. **`Cargo.toml`** lines 19, 22-40 — DX12 backend + extensive Win32 feature flags

**Highest-impact wgpu-fork dependencies (PoC harness)**:
1. **`main.rs`** line 171 — `Renderer::new(dcomp.visual_handle(), hwnd)` uses forked wgpu `CompositionVisual` + `PreMultiplied` alpha APIs
2. **`Cargo.toml`** lines 74-85 — `[patch.crates-io]` section overrides wgpu-hal, wgpu-types, naga with local fork
3. **`sync_wgpu.py`** entire file — git subtree maintenance for `third_party/wgpu` fork

**PoC-specific code (safe to remove for production)**:
1. **`alloc_tracker.rs`** — entire module (debug allocation profiling)
2. **`main.rs`** lines 18, 35-68, 66-68, 82-111, 228-234 — Tracy, alloc_tracker, anti-cheat scan, demo interactive rect
3. **`config.ron`** — PoC overlay settings (can be replaced with user-provided config or defaults)

**Workflow-specific code (keep for development)**:
1. **`tasks.sh`** — git subtree task runner
2. **`sync_wgpu.py`** — git subtree automation for wgpu fork
3. **`Cargo.toml`** lines 46-48 — Tracy profiler integration (optional dev dependency)

---

## Generalization Paths (PoC + Support)

### Minimal Standalone PoC (Low Effort)
- Delete `alloc_tracker.rs`
- Remove Tracy, anti-cheat scan, demo interactive rect from `main.rs`
- Keep Windows/DX12-only; document platform constraints
- Retain wgpu fork until upstream merges premultiplied-alpha support

### Cross-Platform Harness (Medium Effort)
- Replace `overlay_window` calls with winit (cross-platform windowing)
- Replace VK codes in `config.ron` with winit `VirtualKeyCode`
- Add Linux/macOS-specific dependencies to `Cargo.toml` conditionally
- Parameterize wgpu backend (Vulkan/Metal/DX12) via feature flags
- Keep fork + sync_wgpu.py until upstream wgpu merge

### Minimal Library Example (Low Effort)
- Extract harness into `glass-poc/examples/minimal.rs` (standalone example)
- Remove `glass-poc` crate from workspace; keep as example-only
- Document "bring your own window" integration pattern
- Retain fork until upstream wgpu merge

---

## 3. Classification and Generalization Roadmap

This section provides explicit classification, generalization paths, effort estimates, and breaking-change callouts for all audited modules and files from Sections 1 and 2.

**Taxonomy**:
- **specific-only**: Intentionally platform/backend-specific; keep as-is or provide multi-platform alternatives behind traits/conditional compilation
- **quick-cleanup**: PoC artifacts or trivial refactors (remove test scaffolding, replace hardcoded constants)
- **significant-rework**: Requires new abstractions, cross-platform bindings, or major API changes

---

### 3.1 Glass-Overlay Modules

#### compositor.rs
- **Classification**: **specific-only** (Windows DirectComposition)
- **Current state**: Entire module is Windows-only; DirectComposition is the premultiplied-alpha transparency mechanism
- **Generalization path**:
  - **Option A (trait-based)**: Define `trait CompositorBackend` with methods `create_visual() -> NonNull<c_void>`, `commit()`. Provide per-platform implementations:
    - Windows: Keep existing DirectComposition (lines 1-75)
    - Linux/X11: Use `_NET_WM_WINDOW_OPACITY` + shaped windows (requires `xcb` or `x11` crate)
    - Linux/Wayland: Use `zwlr_layer_shell_v1` or `xdg_toplevel` with alpha compositor (requires `wayland-client` crate)
    - macOS: Use `NSWindow` with `alphaValue` + `opaque = false` (requires `cocoa` crate)
  - **Option B (Windows-only initially)**: Document platform constraint in README; stub out compositor on non-Windows platforms (returns no-op visual handle)
- **Effort estimate**: **8-12 hours** (Option A), **1-2 hours** (Option B)
- **Breaking changes**: ✅ Yes — `Compositor::new()` signature may change if platform-specific context is required (e.g., X11 `Display*` or Wayland `wl_compositor*`)
- **Evidence**: Lines 1-75 (Windows-only); lines 4-6, 60 (wgpu fork dependency for `CompositionVisual` enum variant)
- **Recommended approach**: **Option B** for MVP (document Windows-only); **Option A** for cross-platform release

---

#### renderer.rs
- **Classification**: **quick-cleanup** (PoC shader/test artifacts) + **significant-rework** (window handle abstraction)
- **Current state**: 
  - Lines 24-93: Hardcoded WGSL triangle shader (PoC demo geometry)
  - Lines 54-93: Conditional test-mode watermark shader variant
  - Lines 268-285: Test-mode watermark text nodes
  - Lines 406-411: Hardcoded `rpass.draw(0..vertex_count, 0..1)` with 3 or 9 vertices
  - Line 18-19, 120, 128-135: Win32 HWND + `GetClientRect` for sizing
  - Line 139: Hardcoded `wgpu::Backends::DX12`
  - Lines 145-150: `wgpu::SurfaceTargetUnsafe::CompositionVisual(visual.as_ptr())` (forked wgpu API)
  - Lines 187-193: `wgpu::CompositeAlphaMode::PreMultiplied` (forked wgpu API)
- **Generalization path**:
  1. **Quick cleanup (PoC artifacts)**:
     - Delete lines 24-93 (hardcoded shader), 54-93 (test-mode shader variant), 268-285 (test watermark), 406-411 (triangle draw call)
     - Replace with blank/user-provided scene geometry or document "bring your own shader" pattern
  2. **Window handle abstraction**:
     - Replace `HWND` parameter with `raw_window_handle::RawWindowHandle` (cross-platform trait)
     - Replace `GetClientRect` with platform-agnostic sizing (e.g., from `winit::Window::inner_size()` or pass dimensions explicitly)
  3. **Backend parameterization**:
     - Replace hardcoded `wgpu::Backends::DX12` with runtime or compile-time selection (`Backends::PRIMARY` or feature-flag-driven)
     - Conditional compilation for `CompositionVisual` (Windows-only wgpu fork API)
- **Effort estimate**: **2-3 hours** (cleanup), **4-6 hours** (window handle abstraction + backend parameterization)
- **Breaking changes**: ✅ Yes — `Renderer::new()` signature changes from `(NonNull<c_void>, HWND)` to `(NonNull<c_void>, RawWindowHandle)` or `(NonNull<c_void>, u32, u32)` (width, height)
- **Evidence**: Lines 18-19, 24-93, 120, 128-135, 139, 145-150, 187-193, 268-285, 406-411
- **Recommended approach**: Quick cleanup first (remove PoC shader), then window handle abstraction for cross-platform MVP

---

#### overlay_window.rs
- **Classification**: **specific-only** (Windows Win32 HWND lifecycle)
- **Current state**: Entire module (lines 1-552) is Win32-specific — HWND creation, message loop, system tray, DPI awareness, layered windows, hotkey registration
- **Generalization path**:
  - **Option A (winit integration)**: Replace with `winit::Window` (cross-platform windowing library). Changes required:
    - Remove `CreateWindowExW`, `GetMessageW`, `DispatchMessageW` (lines 88-147, 446-530)
    - Replace `SetLayeredWindowAttributes` + `WS_EX_LAYERED | WS_EX_TRANSPARENT` with winit platform-specific extensions (e.g., `winit::platform::windows::WindowBuilderExtWindows::with_no_redirection_bitmap`, `with_decorations(false)`)
    - Replace `Shell_NotifyIconW` system tray with external crate (`tray-icon` or `tao`)
    - Replace `RegisterHotKey` with global hotkey crate (`global-hotkey`)
    - Replace `SetProcessDpiAwarenessContext` with winit's built-in DPI handling
  - **Option B (trait abstraction)**: Define `trait OverlayWindow` with methods `create()`, `run_event_loop()`, `set_input_mode()`. Provide per-platform implementations (Win32, X11, Wayland, Quartz).
  - **Option C (Windows-only initially)**: Document platform constraint; do not port initially
- **Effort estimate**: **16-24 hours** (Option A), **24-32 hours** (Option B), **0 hours** (Option C)
- **Breaking changes**: ✅ Yes — `create_overlay_window()`, `run_message_loop()` APIs replaced entirely; callers must adopt winit or platform-specific trait
- **Evidence**: Lines 1-552 (entire module Win32-specific); lines 40-52, 58-68, 88-147, 151, 193-220, 308-423, 446-530
- **Recommended approach**: **Option C** for MVP (Windows-only), **Option A** for cross-platform release (winit is mature and widely adopted)

---

#### input.rs
- **Classification**: **quick-cleanup** (Win32 message constants) + **specific-only** (timer/hotkey IDs)
- **Current state**: 
  - Lines 17-26: Custom Windows messages `WM_GLASS_MODE_INTERACTIVE` / `WM_GLASS_MODE_PASSIVE` (`WM_APP + 10/11`)
  - Line 23: `INTERACTIVE_TIMER_ID: usize = 42` (Win32 timer ID)
  - Line 25: `HOTKEY_ID: i32 = 1` (Win32 hotkey ID)
- **Generalization path**:
  - Replace Win32 messages with platform-agnostic event enum (e.g., `InputEvent::SetMode(InputMode)`)
  - Abstract timer/hotkey IDs behind cross-platform timer/hotkey crate (`global-hotkey`, `async-timer`)
- **Effort estimate**: **2-4 hours**
- **Breaking changes**: ⚠️ Minor — message constants are internal; external API (`InputMode` enum) remains stable
- **Evidence**: Lines 17-26
- **Recommended approach**: Quick cleanup if porting to winit; keep Win32 constants if staying Windows-only initially

---

#### config.rs
- **Classification**: **quick-cleanup** (Win32 VK codes)
- **Current state**: 
  - Lines 90-97: `hotkey_vk: u32` + `hotkey_modifiers: u32` (Win32 virtual key codes + `MOD_*` flags)
  - Line 107: Default `0x7B` (VK_F12)
- **Generalization path**:
  - Replace `u32` VK codes with cross-platform key enum (e.g., `winit::keyboard::KeyCode` or custom enum)
  - Replace `MOD_*` flags with cross-platform modifier enum (e.g., `winit::keyboard::ModifiersState`)
- **Effort estimate**: **1-2 hours**
- **Breaking changes**: ⚠️ Minor — config file format changes (VK codes → key names); provide migration guide
- **Evidence**: Lines 90-97, 107
- **Recommended approach**: Quick cleanup when adopting cross-platform windowing library (winit provides standard key enums)

---

#### modules/* (mod.rs, clock.rs, fps_counter.rs, system_stats.rs)
- **Classification**: **Already generic** ✅
- **Current state**: Uses cross-platform APIs (`chrono` for clock, `sysinfo` for system stats); no platform-specific dependencies
- **Generalization path**: None required
- **Effort estimate**: **0 hours**
- **Breaking changes**: ❌ None
- **Evidence**: Lines 1-end (all files; no Win32/DXGI/wgpu-specific code)
- **Recommended approach**: Keep as-is

---

#### layout.rs
- **Classification**: **Already generic** ✅
- **Current state**: Generic anchor-based positioning math; no platform-specific dependencies
- **Generalization path**: None required
- **Effort estimate**: **0 hours**
- **Breaking changes**: ❌ None
- **Evidence**: Lines 1-end (no Win32/DXGI/wgpu-specific code)
- **Recommended approach**: Keep as-is

---

#### hdr.rs
- **Classification**: **specific-only** (Windows DXGI HDR detection) + **quick-cleanup** (PoC flag)
- **Current state**: 
  - Lines 1-132: Entire module uses Windows DXGI APIs (`IDXGIFactory1`, `IDXGIOutput6`) for HDR detection
  - Lines 8-10: `use windows::Win32::Graphics::Dxgi::*`
  - Lines 37-72: `CreateDXGIFactory1`, `EnumAdapters1`, `EnumOutputs`, `cast::<IDXGIOutput6>()`
  - Lines 94-95: `DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020` / `DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709` constants
  - Lines 134-166: `choose_surface_format` selects `wgpu::TextureFormat::Rgba16Float` (HDR) or `Bgra8UnormSrgb` (SDR)
  - Line 180: `--force-sdr` command-line flag (PoC override)
- **Generalization path**:
  - **Option A (trait-based)**: Define `trait HdrDetector` with method `detect_hdr_displays() -> Vec<HdrInfo>`. Provide per-platform implementations:
    - Windows: Keep existing DXGI detection (lines 1-132)
    - Linux/Wayland: Use Wayland HDR protocols (`wp_color_management_v1`, `zwlr_output_manager_v1` with colorspace queries)
    - Linux/X11: Use X11 EDID parsing or `_ICC_PROFILE` atom (limited HDR support)
    - macOS: Use Core Graphics `CGDisplayCopyColorSpace` + EDR detection
  - **Option B (stub non-Windows)**: Return `HdrInfo { enabled: false }` on non-Windows platforms initially
  - **Quick cleanup**: Remove `--force-sdr` flag (line 180); replace with proper config option
- **Effort estimate**: **6-10 hours** (Option A), **1 hour** (Option B + cleanup)
- **Breaking changes**: ⚠️ Minor — `detect_hdr()` function signature stable; internal DXGI types hidden
- **Evidence**: Lines 1-132, 8-10, 37-72, 94-95, 134-166, 180
- **Recommended approach**: **Option B** for MVP (Windows-only HDR); **Option A** for cross-platform HDR support

---

#### safety.rs
- **Classification**: **specific-only** (Windows process/service queries) + **quick-cleanup** (PoC AC signatures)
- **Current state**: 
  - Lines 192-217: `win32_service_exists` — `OpenSCManagerW` + `OpenServiceW` (Windows Services API)
  - Lines 223-264: `win32_process_running` — `CreateToolhelp32Snapshot` + `Process32FirstW/NextW` (Windows process enumeration)
  - Line 179: `Path::new("C:\\Windows\\System32\\drivers\\{name}")` (Windows driver path)
  - Lines 120-156: `AC_SIGNATURES` data — detects Vanguard, Ricochet, EAC, BattlEye, VAC (research-specific safety checks)
- **Generalization path**:
  - **Option A (cross-platform process query)**: Abstract process/service checks per platform:
    - Windows: Keep existing Win32 APIs (lines 192-264)
    - Linux: Parse `/proc` filesystem (`/proc/[pid]/cmdline`, `/proc/modules` for kernel drivers)
    - macOS: Use `libproc` APIs (`proc_listallpids`, `proc_pidpath`)
  - **Option B (remove entirely)**: Delete module; anti-cheat detection is research-specific, not required for production overlay
  - **Quick cleanup**: Remove `AC_SIGNATURES` data (lines 120-156) if keeping module; replace with user-provided signature config
- **Effort estimate**: **8-12 hours** (Option A), **0.5 hours** (Option B)
- **Breaking changes**: ⚠️ Minor if removing (callers must handle absence of anti-cheat scan); ❌ None if keeping Windows-only
- **Evidence**: Lines 120-156, 179, 192-264
- **Recommended approach**: **Option B** for MVP (remove module; research-specific feature); **Option A** if cross-platform anti-cheat detection is required

---

#### scene.rs
- **Classification**: **Already generic** ✅
- **Current state**: Pure scene graph data structure; no platform-specific dependencies
- **Generalization path**: None required
- **Effort estimate**: **0 hours**
- **Breaking changes**: ❌ None
- **Evidence**: Lines 1-end (no Win32/DXGI/wgpu-specific code)
- **Recommended approach**: Keep as-is

---

#### text_renderer.rs
- **Classification**: **Already generic** ✅ (glyphon is cross-platform)
- **Current state**: Uses `glyphon` crate (cross-platform text rendering); depends on wgpu device/queue but format-agnostic
- **Generalization path**: None required (glyphon supports Vulkan/Metal/DX12/GL via wgpu)
- **Effort estimate**: **0 hours**
- **Breaking changes**: ❌ None
- **Evidence**: Lines 1-end (glyphon is cross-platform); lines 46-66 (wgpu primitives are backend-agnostic)
- **Recommended approach**: Keep as-is

---

#### test_mode.rs
- **Classification**: **quick-cleanup** (PoC validation feature)
- **Current state**: 
  - Lines 1-50: Entire module — defines PoC watermark text, forced passthrough mode, window title prefix for test builds
  - Lines 22-26: `WATERMARK_LINES` — French research prototype notice
  - Line 35: `FORCE_INPUT_PASSTHROUGH: bool = true` — disables interactive hotkeys in test mode
- **Generalization path**: Delete module entirely (PoC validation feature only)
- **Effort estimate**: **0.5 hours** (delete file + remove references in `renderer.rs`, `overlay_window.rs`)
- **Breaking changes**: ⚠️ Minor — callers must handle absence of test-mode watermark; `#[cfg(feature = "test_mode")]` blocks must be removed
- **Evidence**: Lines 1-50
- **Recommended approach**: Delete module for production release

---

#### diagnostics.rs
- **Classification**: **specific-only** (Windows diagnostics APIs) + **quick-cleanup** (PoC debug feature)
- **Current state**: 
  - Lines 112-127: `OsInfo::capture` — `GetVersionExW` (deprecated Win32 version API)
  - Lines 131-158: `GpuInfo::capture` — `CreateDXGIFactory1`, `EnumAdapters1`, `GetDesc1` (DXGI adapter info)
  - Lines 161-169: `DwmInfo::capture` — `DwmIsCompositionEnabled` (DWM state query)
  - Lines 173-227: `OutputInfo::enumerate` — DXGI output enumeration + HDR detection via `IDXGIOutput6`
  - Lines 231-254: `ProcessInfo::capture` — `GetWindowDpiAwarenessContext`, `GetAwarenessFromDpiAwarenessContext`, `GetDpiForWindow` (Windows DPI APIs)
  - Lines 1-265: Entire module — GPU diagnostics dump for research/debugging (captures error context and color pipeline name for triage)
- **Generalization path**:
  - **Option A (cross-platform diagnostics)**: Abstract diagnostics per platform:
    - Windows: Keep existing DXGI/DWM/DPI queries (lines 112-254)
    - Linux: Parse `/sys/class/drm`, `/sys/class/backlight`, `/proc/cpuinfo`, `/proc/meminfo`
    - macOS: Use `system_profiler SPDisplaysDataType`, `ioreg`, `sysctl`
  - **Option B (remove entirely)**: Delete module; diagnostics are research-specific, not required for production overlay
- **Effort estimate**: **12-16 hours** (Option A), **0.5 hours** (Option B)
- **Breaking changes**: ⚠️ Minor if removing (callers must handle absence of diagnostics capture); ❌ None if keeping Windows-only
- **Evidence**: Lines 1-265, 112-254
- **Recommended approach**: **Option B** for MVP (remove module; research-specific feature); **Option A** if cross-platform diagnostics are required for user support/triage

---

### 3.2 Glass-PoC and Support Files

#### main.rs (glass-poc/src/main.rs)
- **Classification**: **quick-cleanup** (PoC features) + **significant-rework** (window abstraction)
- **Current state**: 
  - Line 18: `mod alloc_tracker` — debug allocation profiling (PoC validation)
  - Lines 35-38: Feature-gated `test_mode` vs `info` default tracing filter
  - Lines 40-64: Tracy profiling integration (`#[cfg(feature = "tracy")]`)
  - Lines 66-68: `alloc_tracker::install()` — debug-only allocation tracking
  - Lines 82-111: Anti-cheat self-check (`AntiCheatDetector::scan()`) — passive kernel-AC detection before init; blocks startup if kernel AC detected
  - Lines 228-234: Demo interactive rect at (100, 100) 200×60 — PoC hit-testing example
  - Lines 28, 76, 97, 114, 143-178: Win32 overlay_window, DComp, HWND lifecycle
  - Line 171: `Renderer::new(dcomp.visual_handle(), hwnd)` — uses forked wgpu APIs
- **Generalization path**:
  1. **Quick cleanup (PoC features)**:
     - Delete lines 18, 40-68 (alloc_tracker, Tracy integration)
     - Delete lines 82-111 (anti-cheat scan)
     - Delete lines 228-234 (demo interactive rect)
     - Replace lines 35-38 with standard tracing filter (remove test_mode conditional)
  2. **Window abstraction** (cross-platform):
     - Replace `overlay_window` calls (lines 76, 97, 114, 143-147) with `winit::Window` or equivalent
     - Replace `Compositor::new(hwnd)` + `Renderer::new(dcomp.visual_handle(), hwnd)` with platform-agnostic surface creation (e.g., `raw_window_handle::HasRawWindowHandle`)
  3. **Extract to example**: Move harness to `glass-poc/examples/minimal.rs`; remove `glass-poc` from default workspace members
- **Effort estimate**: **2-3 hours** (cleanup), **8-12 hours** (window abstraction), **1 hour** (extract to example)
- **Breaking changes**: ✅ Yes if extracting to example (binary crate → example-only); ⚠️ Minor if keeping as binary (internal PoC features removed)
- **Evidence**: Lines 18, 28, 35-111, 143-178, 228-234
- **Recommended approach**: Quick cleanup first (remove PoC features), then extract to example for minimal standalone harness

---

#### alloc_tracker.rs (glass-poc/src/alloc_tracker.rs)
- **Classification**: **quick-cleanup** (PoC validation feature)
- **Current state**: 
  - Lines 1-80: Entire module — debug-mode per-frame allocation counter (PoC validation; warns if steady-state frames trigger heap allocations)
  - Lines 45-47: `#[global_allocator]` static override (PoC diagnostics)
  - Lines 10-13: `ALLOC_COUNT`, `INSTALLED` atomics for frame profiling
- **Generalization path**: Delete module entirely (PoC validation feature only)
- **Effort estimate**: **0.5 hours** (delete file + remove reference in `main.rs` line 18, 66-68)
- **Breaking changes**: ⚠️ Minor — `#[global_allocator]` override removed; default allocator restored
- **Evidence**: Lines 1-80
- **Recommended approach**: Delete module for production release

---

#### Cargo.toml (workspace root)
- **Classification**: **quick-cleanup** (PoC structure) + **significant-rework** (multi-backend support)
- **Current state**: 
  - Lines 3-7: Workspace members: `glass-core`, `glass-overlay`, `glass-poc`
  - Lines 8-10: `exclude = ["third_party/wgpu"]` — subtree directory
  - Lines 13-14: Workspace metadata: `edition = "2024"`, `rust-version = "1.85"` (requires nightly Rust 2024 edition)
  - Lines 19, 22-40: `wgpu` with `features = ["dx12"]` — hardcoded DX12 backend; `windows` crate with extensive Win32 feature flags
  - Lines 74-85: `[patch.crates-io]` section — overrides wgpu-hal, wgpu-types, naga with local fork in `third_party/wgpu`
  - Lines 46-48: `tracing-tracy` dependency — Tracy profiler integration
  - Lines 51-52, 63: `pollster`, `notify` dependencies — async blocking + config hot-reload
- **Generalization path**:
  1. **Quick cleanup (PoC structure)**:
     - Remove `glass-poc` from default workspace members (lines 3-7); make example-only
     - Remove or feature-gate `tracing-tracy` (lines 46-48)
     - Consider stabilizing edition to "2021" if Rust 2024 is not required (line 13)
  2. **Multi-backend support**:
     - Replace `features = ["dx12"]` with multi-backend feature flags: `features = ["dx12", "vulkan", "metal"]` (line 19)
     - Add conditional Win32 dependencies: `[target.'cfg(windows)'.dependencies]` for `windows` crate (lines 22-40)
     - Add Linux/macOS dependencies conditionally: `[target.'cfg(target_os = "linux")'.dependencies]` for `x11`/`wayland` crates
  3. **Fork maintenance**:
     - Retain `[patch.crates-io]` section (lines 74-85) until wgpu upstream merges premultiplied-alpha support
     - Document fork rationale in README (lines 75-78 comment already explains)
- **Effort estimate**: **2-3 hours** (cleanup), **4-6 hours** (multi-backend support)
- **Breaking changes**: ⚠️ Minor — workspace structure changes; downstream consumers must update dependency paths if `glass-poc` is removed
- **Evidence**: Lines 3-14, 19, 22-48, 74-85
- **Recommended approach**: Quick cleanup first (remove Tracy, stabilize edition); multi-backend support when porting to cross-platform windowing

---

#### config.ron (root)
- **Classification**: **quick-cleanup** (Win32 VK codes) + **specific-only** (PoC config file)
- **Current state**: 
  - Lines 15-16: `hotkey_vk: 0x7B` (F12), `hotkey_modifiers: 0` — Win32 virtual key code + `MOD_*` flags
  - Lines 1-20: Entire file — PoC overlay configuration (position, size, opacity, colors, hotkey, timeout)
- **Generalization path**:
  - Replace VK codes (lines 15-16) with cross-platform key names (e.g., `hotkey: "F12"`, `modifiers: []`)
  - Keep position/size/opacity/colors (generic widget config)
  - Consider moving to user config directory (`~/.config/glass/config.ron` or `%APPDATA%\glass\config.ron`) instead of repo root
- **Effort estimate**: **1-2 hours**
- **Breaking changes**: ⚠️ Minor — config file format changes; provide migration guide (VK codes → key names)
- **Evidence**: Lines 1-20, 15-16
- **Recommended approach**: Quick cleanup when adopting cross-platform windowing; move to user config directory for production

---

#### clippy.toml
- **Classification**: **Already generic** ✅
- **Current state**: Standard Clippy config with `msrv = "1.85"`
- **Generalization path**: None required
- **Effort estimate**: **0 hours**
- **Breaking changes**: ❌ None
- **Evidence**: Line 1
- **Recommended approach**: Keep as-is

---

#### tasks.sh
- **Classification**: **Already generic** ✅ (git subtree workflow)
- **Current state**: Task runner for wgpu subtree sync workflow (delegates to `sync_wgpu.py`)
- **Generalization path**: None required (standard git subtree operations)
- **Effort estimate**: **0 hours**
- **Breaking changes**: ❌ None
- **Evidence**: Lines 1-21
- **Recommended approach**: Keep as-is until wgpu fork is retired

---

#### sync_wgpu.py
- **Classification**: **Already generic** ✅ (git subtree automation) + **specific-only** (wgpu fork maintenance)
- **Current state**: 
  - Lines 1-253: Entire file — manages git subtree for `third_party/wgpu` fork
  - Lines 27-32: Configuration constants: `WGPU_FORK_URL`, `DEFAULT_BRANCH = "v24"` (tracks v24 series with premultiplied-alpha patches)
  - Lines 96-219: Commands: `status`, `setup`, `pull`, `push` — standard git subtree operations
- **Generalization path**: 
  - Keep script until wgpu upstream merges premultiplied-alpha support
  - Remove script + `third_party/wgpu` subtree once upstream wgpu can be used directly from crates.io
  - Update `Cargo.toml` to remove `[patch.crates-io]` section when fork is retired
- **Effort estimate**: **0 hours** (keep until upstream merge), **1 hour** (remove script + subtree + patch when upstream is ready)
- **Breaking changes**: ❌ None (fork is transparent to downstream consumers via `[patch.crates-io]`)
- **Evidence**: Lines 1-253, 27-32, 96-219
- **Recommended approach**: Keep script + fork for MVP; monitor wgpu upstream PRs for premultiplied-alpha merge; retire fork once upstream is ready

---

### 3.3 Summary Tables

#### By Taxonomy

| Category              | Count | Modules/Files |
|-----------------------|-------|---------------|
| **specific-only**     | 6     | `compositor.rs`, `overlay_window.rs`, `hdr.rs`, `safety.rs`, `diagnostics.rs`, `sync_wgpu.py` |
| **quick-cleanup**     | 7     | `renderer.rs` (PoC shader), `input.rs`, `config.rs`, `test_mode.rs`, `alloc_tracker.rs`, `main.rs` (PoC features), `config.ron` |
| **significant-rework**| 3     | `renderer.rs` (window handle), `overlay_window.rs` (if cross-platform), `main.rs` (window abstraction) |
| **already-generic**   | 6     | `modules/*`, `layout.rs`, `scene.rs`, `text_renderer.rs`, `clippy.toml`, `tasks.sh` |

#### By Effort Estimate (Person-Hours)

| Effort Range  | Count | Modules/Files |
|---------------|-------|---------------|
| **0 hours** (already generic) | 6 | `modules/*`, `layout.rs`, `scene.rs`, `text_renderer.rs`, `clippy.toml`, `tasks.sh` |
| **0.5-2 hours** (trivial cleanup) | 6 | `test_mode.rs`, `alloc_tracker.rs`, `input.rs`, `config.rs`, `config.ron`, `Cargo.toml` (cleanup) |
| **2-6 hours** (quick refactor) | 3 | `renderer.rs` (cleanup + window handle), `main.rs` (cleanup), `Cargo.toml` (multi-backend) |
| **6-12 hours** (medium rework) | 4 | `compositor.rs` (trait-based), `hdr.rs` (trait-based), `safety.rs` (cross-platform), `main.rs` (window abstraction) |
| **12-24+ hours** (significant rework) | 3 | `overlay_window.rs` (winit), `diagnostics.rs` (cross-platform), `compositor.rs` (multi-platform) |

#### Breaking Changes Callout

| Module/File           | Breaking Change | Impact |
|-----------------------|-----------------|--------|
| `compositor.rs`       | ✅ Yes          | `Compositor::new()` signature may change if platform-specific context is required |
| `renderer.rs`         | ✅ Yes          | `Renderer::new()` signature changes from `(NonNull<c_void>, HWND)` to `(NonNull<c_void>, RawWindowHandle)` or `(NonNull<c_void>, u32, u32)` |
| `overlay_window.rs`   | ✅ Yes          | `create_overlay_window()`, `run_message_loop()` APIs replaced entirely; callers must adopt winit or platform-specific trait |
| `input.rs`            | ⚠️ Minor        | Internal Win32 message constants change; external `InputMode` enum remains stable |
| `config.rs`           | ⚠️ Minor        | Config file format changes (VK codes → key names); provide migration guide |
| `test_mode.rs`        | ⚠️ Minor        | Callers must handle absence of test-mode watermark; `#[cfg(feature = "test_mode")]` blocks removed |
| `alloc_tracker.rs`    | ⚠️ Minor        | `#[global_allocator]` override removed; default allocator restored |
| `main.rs`             | ✅ Yes          | If extracted to example, binary crate → example-only; internal PoC features removed |
| `Cargo.toml`          | ⚠️ Minor        | Workspace structure changes; downstream consumers must update dependency paths if `glass-poc` is removed |
| `config.ron`          | ⚠️ Minor        | Config file format changes (VK codes → key names); provide migration guide |
| All others            | ❌ None         | No breaking changes expected |

---

### 3.4 Recommended Phased Approach

**Phase 1: Quick Wins (MVP — Windows-only, ~8-12 hours total)**
1. Delete `test_mode.rs`, `alloc_tracker.rs`, `diagnostics.rs`, `safety.rs` (PoC validation features) — **2 hours**
2. Remove hardcoded triangle shader + test watermark from `renderer.rs` — **1 hour**
3. Remove Tracy integration, anti-cheat scan, demo rect from `main.rs` — **1 hour**
4. Extract `main.rs` to `glass-poc/examples/minimal.rs`; remove `glass-poc` from default workspace members — **1 hour**
5. Replace VK codes in `config.rs` + `config.ron` with cross-platform key names (winit enums) — **2 hours**
6. Update README with "Windows-only MVP" constraints + fork rationale — **2 hours**
7. Validation: `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace` — **1 hour**

**Deliverable**: Clean, documented, Windows-only MVP with PoC artifacts removed; ~8-12 hours total effort.

**Phase 2: Cross-Platform Windowing (~20-30 hours total)**
1. Replace `overlay_window.rs` with winit (cross-platform window creation, event loop, DPI handling) — **16-20 hours**
2. Replace `HWND` in `renderer.rs` with `raw_window_handle::RawWindowHandle` — **4-6 hours**
3. Abstract Win32 messages in `input.rs` with platform-agnostic events — **2-4 hours**
4. Conditional compilation for `CompositionVisual` (Windows-only wgpu fork API) — **2 hours**
5. Validation: cross-compile for Linux/macOS (may require stubs for compositor/HDR) — **2-4 hours**

**Deliverable**: Cross-platform windowing with Windows compositor + DX12 backend; Linux/macOS window creation works but compositor/HDR are stubbed; ~20-30 hours total effort.

**Phase 3: Multi-Backend + Multi-Platform Compositor (~40-60 hours total)**
1. Parameterize wgpu backend selection (Vulkan/Metal/DX12/GL) via feature flags — **4-6 hours**
2. Implement trait-based compositor abstraction (Windows: DComp, Linux: X11/Wayland, macOS: Quartz) — **24-32 hours**
3. Implement trait-based HDR detection (Windows: DXGI, Linux: Wayland protocols, macOS: Core Graphics) — **8-12 hours**
4. Conditional dependencies in `Cargo.toml` (Win32, X11, Wayland, Cocoa crates) — **4-6 hours**
5. Validation: test on Windows, Linux (X11 + Wayland), macOS — **8-12 hours**

**Deliverable**: Fully cross-platform overlay with transparent compositing + HDR support on Windows/Linux/macOS; ~40-60 hours total effort.

**Phase 4: Retire wgpu Fork (~2-4 hours total)**
1. Monitor wgpu upstream PRs for premultiplied-alpha merge — **ongoing**
2. Remove `[patch.crates-io]` from `Cargo.toml` once upstream is ready — **1 hour**
3. Delete `third_party/wgpu` subtree + `sync_wgpu.py` script — **1 hour**
4. Update README to remove fork rationale section — **1 hour**
5. Validation: ensure upstream wgpu works with compositor — **1-2 hours**

**Deliverable**: wgpu fork retired; upstream wgpu used directly from crates.io; ~2-4 hours total effort.

---

## End of Section 3
