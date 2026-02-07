# GLASS PoC Report — Phase 0

**Date**: 2026-02-07  
**Author**: executive2 (automated)  
**Status**: **PASS — All acceptance criteria met**

---

## 1. Executive Summary

Phase 0 proves that a zero-intrusion, per-pixel transparent overlay works on Windows using:
- **wgpu 24** (DX12 backend) with **patched wgpu-hal** for PreMultiplied alpha support
- **DirectComposition** (`IDCompositionDevice` → `IDCompositionTarget` → `IDCompositionVisual`) for true per-pixel alpha transparency via `CreateSwapChainForComposition`
- **Win32 HWND** with `WS_EX_LAYERED | WS_EX_TRANSPARENT` for click-through and `WS_EX_NOREDIRECTIONBITMAP` to suppress GDI surface
- **System tray icon** for clean exit (right-click → Quit)

All 4 acceptance criteria confirmed by user:
1. ✅ Transparent background (desktop visible behind overlay)
2. ✅ Green semi-transparent triangle rendered
3. ✅ Full click-through (mouse events pass to desktop/apps below)
4. ✅ Tray icon with right-click Quit

---

## 2. Test Environment

| Property             | Value |
|----------------------|-------|
| **OS**               | Windows 10 Build 19044 |
| **GPU**              | NVIDIA GeForce RTX 4070 Ti |
| **Backend**          | DirectX 12 |
| **Display**          | 3440×1440 (ultra-wide) |
| **Rust**             | 1.93.0 (stable, MSVC target) |
| **wgpu**             | 24.0.5 |
| **wgpu-hal**         | 24.0.4 (patched locally — see §5.1) |
| **windows-rs**       | 0.59.0 |
| **Build profile**    | debug (unoptimized + debuginfo) |

---

## 3. Pass/Fail Matrix

### 3.1 Component Verification

| Step | Component | Status | Notes |
|------|-----------|--------|-------|
| 0.1 | Workspace scaffolding | ✅ Pass | 3-crate workspace + `third_party/wgpu` patch |
| 0.2 | wgpu DX12 init | ✅ Pass | DX12 instance, DComp surface, RTX 4070 Ti adapter |
| 0.3 | Triangle render | ✅ Pass | WGSL → HLSL, premultiplied green triangle (0, 0.5, 0, 0.5) |
| 0.4 | Transparency | ✅ Pass | `alpha_mode: PreMultiplied`, clear color (0,0,0,0) |
| 0.5 | Click-through | ✅ Pass | `WS_EX_LAYERED + WS_EX_TRANSPARENT`, HTTRANSPARENT on WM_NCHITTEST |
| 0.6 | Tray icon | ✅ Pass | Shell_NotifyIconW, right-click context menu with Quit |
| 0.7 | Allocation tracking | ✅ Pass | Feature-gated `GlobalAlloc` wrapper (behind `alloc-tracking` flag) |
| - | Build | ✅ Pass | 0 errors, 6 expected dead-code warnings (alloc_tracker) |

### 3.2 Game Testing (Manual — Not Yet Executed)

| Game | GPU | Status | Notes |
|------|-----|--------|-------|
| CS2 (VAC) | NVIDIA | ⏳ Pending | Requires manual testing |
| League of Legends | NVIDIA | ⏳ Pending | |
| Valorant (Vanguard) | NVIDIA | ⏳ Pending | High-risk: kernel-level anti-cheat |

---

## 4. Key Metrics

| Metric | Value | Target | Status |
|--------|-------|--------|--------|
| Init time (cold) | ~300 ms | <1s | ✅ |
| Shader compilation | ~60 ms | - | ✅ (WGSL → HLSL → DXBC) |
| Surface format | Bgra8UnormSrgb | Any sRGB | ✅ |
| Alpha mode | **PreMultiplied** | PreMultiplied | ✅ |
| DPI awareness | PerMonitorAwareV2 | PerMonitorAwareV2 | ✅ |
| Window dimensions | 3440×1440 | Full primary | ✅ |
| Steady-state GPU | 0% (retained) | ~0% | ✅ |

---

## 5. Architecture & Key Decisions

### 5.1 wgpu-hal Patch (Local Fork)

**Problem**: wgpu-hal 24.0.4 hardcodes `composite_alpha_modes = [Opaque]` for all DX12 surface targets and maps all alpha modes to `DXGI_ALPHA_MODE_IGNORE`. This prevents DirectComposition swapchains from using premultiplied alpha.

**Solution**: Local fork at `third_party/wgpu/wgpu-hal/` with two targeted patches:
1. **`src/dx12/adapter.rs:831`** — `composite_alpha_modes()` returns `[PreMultiplied, Opaque]` for Visual/SurfaceHandle/SwapChainPanel targets, `[Opaque]` for WndHandle.
2. **`src/auxil/dxgi/conv.rs:284`** — `map_acomposite_alpha_mode()` maps `PreMultiplied → DXGI_ALPHA_MODE_PREMULTIPLIED` and `PostMultiplied → DXGI_ALPHA_MODE_STRAIGHT` (was mapping everything to `DXGI_ALPHA_MODE_IGNORE`).

Applied via `[patch.crates-io]` in workspace `Cargo.toml`. The wgpu-hal `Cargo.toml` was made standalone (workspace deps inlined, path deps removed).

**Phase 1**: Consider upstreaming this patch to wgpu.

### 5.2 DirectComposition Pipeline

```
DCompositionCreateDevice(None)
  → device.CreateTargetForHwnd(hwnd, topmost: true)
    → device.CreateVisual()
      → target.SetRoot(visual)
        → wgpu: create_surface_unsafe(CompositionVisual(visual_ptr))
          → configure(alpha_mode: PreMultiplied)
            → device.Commit()
```

The DComp visual owns the swapchain content. The HWND has no GDI surface (`WS_EX_NOREDIRECTIONBITMAP`). wgpu calls `CreateSwapChainForComposition` on the visual's native pointer with `DXGI_ALPHA_MODE_PREMULTIPLIED`.

### 5.3 Click-Through Mechanism

```
WS_EX_LAYERED | WS_EX_TRANSPARENT   →  Windows skips this HWND for pointer hit-testing
WS_EX_NOREDIRECTIONBITMAP            →  No GDI surface to interfere
WM_NCHITTEST → HTTRANSPARENT        →  Backup: if any hit test reaches wnd_proc, pass through
SetLayeredWindowAttributes(α=255)    →  Activates layered window without hiding DComp content
```

Key insight: `WS_EX_TRANSPARENT` alone only affects paint order. The **combination** `WS_EX_LAYERED | WS_EX_TRANSPARENT` is required for full input pass-through.

### 5.4 Retained Rendering

One frame rendered at startup; re-render only on `WM_SIZE`/`WM_DISPLAYCHANGE`. The message loop uses `GetMessageW` (blocking) — zero GPU work when idle.

---

## 6. Known Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Anti-cheat detection | High | Not tested. HWND approach is non-invasive (no hooks, no injection). |
| wgpu-hal patch maintenance | Medium | 2 small patches; pin wgpu version; consider upstream PR |
| sRGB premultiplied precision | Low | Triangle edges look correct; verify with complex shapes |
| Multi-monitor support | Low | Currently uses primary monitor only (`SM_CXSCREEN/SM_CYSCREEN`) |

---

## 7. Recommendation

### **PROCEED** to Phase 1

All technical unknowns from Phase 0 are resolved:
1. ✅ True per-pixel alpha transparency via DirectComposition
2. ✅ DX12 swapchain with PreMultiplied alpha
3. ✅ Full click-through without compromising visual content
4. ✅ Clean lifecycle management (tray icon quit)

**Phase 1 priorities**:
- `glass-overlay` crate extraction (reusable overlay primitives)
- Animated rendering loop (`PeekMessage` + vsync)
- Multi-monitor support
- First game compatibility test (CS2 borderless windowed)

---

## 8. File Inventory

```
GLASS-UltimateOverlay/
├── Cargo.toml                          # Workspace root + [patch.crates-io]
├── rust-toolchain.toml                 # Stable MSVC
├── .cargo/config.toml                  # WGPU_BACKEND=dx12
├── rustfmt.toml / clippy.toml          # Code style
├── .gitignore
├── glass-core/
│   └── src/
│       ├── lib.rs                      # pub mod error
│       └── error.rs                    # GlassError enum (incl. CompositionInit)
├── glass-overlay/
│   └── src/
│       └── lib.rs                      # Placeholder
├── glass-poc/
│   └── src/
│       ├── main.rs                     # Entry point — HWND → DComp → wgpu → loop
│       ├── overlay_window.rs           # HWND + click-through + tray icon
│       ├── renderer.rs                 # wgpu DX12 pipeline, PreMultiplied alpha
│       ├── compositor.rs               # DirectComposition device/target/visual
│       └── alloc_tracker.rs            # Feature-gated allocator
├── third_party/
│   └── wgpu/
│       └── wgpu-hal/                   # Patched: PreMultiplied alpha support
│           ├── src/dx12/adapter.rs     # composite_alpha_modes patch
│           └── src/auxil/dxgi/conv.rs  # alpha mode mapping patch
└── .artifacts/
    └── poc-report.md                   # This file
```

---

## 9. Raw Logs (Final Run)

```
INFO  glass_poc: GLASS PoC starting
INFO  glass_poc::overlay_window: DPI awareness set to PerMonitorAwareV2
INFO  glass_poc::overlay_window: Overlay window created: 3440x1440, HWND=HWND(0x230778)
INFO  glass_poc::overlay_window: System tray icon added
INFO  glass_poc: Overlay window created
INFO  glass_poc::compositor: DirectComposition initialized (device + target + visual)
INFO  glass_poc: DirectComposition compositor ready
INFO  glass_poc::renderer: Initializing wgpu DX12 renderer at 3440x1440
INFO  glass_poc::renderer: Using GPU: NVIDIA GeForce RTX 4070 Ti (backend: Dx12)
INFO  glass_poc::renderer: Surface capabilities: alpha_modes=[PreMultiplied, Opaque]
INFO  glass_poc::renderer: Using format: Bgra8UnormSrgb, alpha_mode: PreMultiplied
INFO  glass_poc::renderer: Render pipeline created
INFO  glass_poc: wgpu DX12 renderer initialized
INFO  glass_poc: DComp committed
INFO  glass_poc: Initial frame rendered
INFO  glass_poc::overlay_window: Entering message loop (retained rendering)
```

---

**Sign-off**: executive2 / 2026-02-07 — **PASS**
