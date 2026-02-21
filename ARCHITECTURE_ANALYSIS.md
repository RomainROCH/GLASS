# GLASS Workspace Architecture Analysis

**Purpose**: Assess architectural complexity for planning a genericity audit, architecture README, and minimal standalone example.

---

## Executive Summary

### Complexity Assessment: **MEDIUM-HIGH**

**Architecture decisions are NON-TRIVIAL** due to:
1. **Tight Windows Platform Coupling** — DirectComposition + Win32 + DX12 APIs
2. **Custom wgpu Fork** — Patched wgpu-hal for premultiplied alpha support
3. **Cross-Crate Abstraction Leakage** — Scene graph, modules, and rendering tightly coupled
4. **Feature Flag Conditionals** — test_mode, tracy, alloc-tracking with scattered #[cfg]
5. **Hot-Reload Configuration** — Arc<ArcSwap> + notify filesystem watcher threading concerns

### Key Risks for Planning

| Risk | Impact | Mitigation |
|------|--------|------------|
| **wgpu subtree dependency** | HIGH | Minimal example must replicate patch or use upstream (limits alpha) |
| **DirectComposition requirement** | HIGH | Cannot isolate without Windows 10+ DComp API surface |
| **Module/Scene/Renderer coupling** | MEDIUM | Clear trait boundaries exist but require all three layers |
| **Feature flag sprawl** | MEDIUM | Document canonical feature combinations |
| **Config hot-reload threading** | LOW | Can stub out or use simpler Arc<Mutex> in minimal example |

---

## 1. Dependency Boundaries (Crate Graph)

### 1.1 Crate Hierarchy

```
glass-poc (binary)
  ├── glass-overlay (library)  [MAIN LOGIC]
  │   ├── glass-core (library)  [MINIMAL ERROR TYPE]
  │   ├── wgpu [PATCHED]
  │   ├── windows
  │   ├── glyphon
  │   ├── sysinfo
  │   ├── notify + arc-swap
  │   └── tracing + tracing-tracy
  └── tracing + tracing-subscriber
```

### 1.2 Crate Responsibilities

#### `glass-core/` (8 LOC total)
- **Role**: Shared error type (`GlassError` enum)
- **Exports**: `pub use error::GlassError;`
- **Dependencies**: Zero (only `tracing` for span instrumentation)
- **Verdict**: **MINIMAL** — Could be inlined into glass-overlay

**Files**:
- `src/lib.rs` (3 lines)
- `src/error.rs` (40 lines) — Error enum with Display impl

#### `glass-overlay/` (Primary crate, ~3000 LOC)
- **Role**: Complete overlay implementation (rendering, input, config, modules)
- **Key Subsystems**:
  - `compositor.rs` (75 lines) — DirectComposition device/target/visual wrapper
  - `renderer.rs` (~500 lines) — wgpu DX12 backend + scene graph rendering
  - `overlay_window.rs` (~600 lines) — Win32 HWND + message pump + tray icon
  - `scene.rs` (~250 lines) — Retained scene graph with dirty-flag tracking
  - `text_renderer.rs` (~200 lines) — Glyphon text engine integration
  - `modules/` (~700 lines) — OverlayModule trait + clock/stats/fps implementations
  - `layout.rs` (~400 lines) — Anchor-based widget positioning system
  - `input.rs` (~350 lines) — Passive/interactive mode + rect-based hit-testing
  - `config.rs` (~300 lines) — Hot-reload RON/TOML config with arc-swap
  - `hdr.rs` (~150 lines) — HDR detection via IDXGIOutput6
  - `safety.rs` (~250 lines) — Passive anti-cheat scanner
  - `test_mode.rs` (20 lines) — Feature-flag watermark constants
  - `diagnostics.rs` (~100 lines) — System diagnostics dump

- **Feature Flags**:
  - `test_mode` — watermark rendering, forced passthrough, TRACE logging
  - `tracy` — Optional profiling via tracing-tracy

#### `glass-poc/` (Thin binary, ~250 LOC)
- **Role**: Bootstrap harness that wires glass-overlay components
- **Files**:
  - `src/main.rs` (250 lines) — Init tracing, anti-cheat check, config load, window/DComp/wgpu init, module registration, message loop
  - `src/alloc_tracker.rs` (optional allocation tracking)

- **Feature Flags**:
  - `alloc-tracking` — Debug-mode allocation instrumentation
  - `test_mode` — Propagates to glass-overlay/test_mode
  - `tracy` — Propagates to glass-overlay/tracy

### 1.3 Critical Dependencies

#### **Patched wgpu** (third_party/wgpu/)
- **Location**: `third_party/wgpu/` (git subtree from fork)
- **Patches Applied**:
  - `wgpu-hal`: Add `DXGI_ALPHA_MODE_PREMULTIPLIED` support for `CreateSwapChainForComposition`
  - Upstream wgpu 24.0.4 hardcodes `DXGI_ALPHA_MODE_IGNORE` (opaque)
- **Workspace Configuration**:
  ```toml
  # Cargo.toml lines 82-85
  [patch.crates-io]
  wgpu-hal   = { path = "third_party/wgpu/wgpu-hal" }
  wgpu-types = { path = "third_party/wgpu/wgpu-types" }
  naga       = { path = "third_party/wgpu/naga" }
  ```
  - **Constraint**: `wgpu-types` must be patched to avoid duplicate type errors between wgpu-core (crates.io) and wgpu-hal (local)
  - **Workspace Exclusion**: `exclude = ["third_party/wgpu"]` prevents cargo from treating the subtree as a workspace member

#### **Windows-rs** (0.59)
- **Features Enabled** (37 features):
  - `Win32_Graphics_DirectComposition` — `IDCompositionDevice`, `DCompositionCreateDevice`
  - `Win32_Graphics_Dxgi` — `IDXGIFactory1`, `IDXGIOutput6` (HDR detection)
  - `Win32_Graphics_Direct3D12` — For DX12 interop (minimal, wgpu handles most)
  - `Win32_UI_WindowsAndMessaging` — `CreateWindowExW`, `DefWindowProcW`, tray icon APIs
  - `Win32_UI_HiDpi` — `SetProcessDpiAwarenessContext`
  - `Win32_System_Diagnostics_ToolHelp` — Anti-cheat process enumeration
  - **18 other features** for window styles, input, COM, etc.

#### **Glyphon** (0.8)
- **Purpose**: GPU-accelerated text rendering (atlas-based, multi-threaded font system)
- **Integration**: Wrapped in `TextEngine` (glass-overlay/src/text_renderer.rs:28-78)
- **Constraint**: Requires wgpu Device/Queue, not trivially swappable

---

## 2. Architecture Patterns & Abstractions

### 2.1 Core Abstractions

#### **Retained Scene Graph** (scene.rs:1-250)
- **Pattern**: Dirty-flag system — nodes are created once, re-uploaded only when modified
- **Node Types**: `SceneNode::Rect`, `SceneNode::Text` (line 75-78)
- **API**:
  ```rust
  scene.add_rect(RectProps { x, y, width, height, color }) -> NodeId
  scene.add_text(TextProps { x, y, text, font_size, color }) -> NodeId
  scene.update_rect(id, props) // Marks dirty
  scene.remove(id)
  scene.drain_dirty() -> impl Iterator<Item = (NodeId, &SceneNode)>
  ```
- **Zero-Allocation Steady State**: No per-frame allocations when scene is static

#### **OverlayModule Trait** (modules/mod.rs:41-81)
- **Lifecycle**:
  ```rust
  fn init(&mut self, scene: &mut Scene);         // Add nodes
  fn update(&mut self, scene: &mut Scene, dt: Duration) -> bool; // Refresh + dirty flag
  fn deinit(&mut self, scene: &mut Scene);       // Remove nodes
  ```
- **Downcasting**: `as_any_mut()` for per-module config updates (line 64)
- **Position Management**: `set_position(x, y)` called by layout system (line 71)
- **Content Sizing**: `content_size() -> (f32, f32)` for anchor resolution (line 78)

#### **Anchor-Based Layout** (layout.rs:1-400)
- **Pattern**: Each widget has an `Anchor` (TopLeft/TopRight/BottomLeft/BottomRight/Center/ScreenPercentage) + margin offsets
- **API**:
  ```rust
  impl Anchor {
      fn resolve(&self, content_w, content_h, screen_w, screen_h, margin_x, margin_y) -> (x, y)
  }
  ```
- **Widget Wrapper**: Composes `OverlayModule` with `Anchor` (line 6)
- **Recalculation**: On `WM_SIZE` / `WM_DISPLAYCHANGE`, recalculate all positions → deinit + reinit moved modules

#### **Hot-Reload Config** (config.rs:1-300)
- **Pattern**: `arc-swap::ArcSwap<Config>` for lock-free reads from render loop
- **Watcher**: `notify::RecommendedWatcher` on background thread posts reload events
- **API**:
  ```rust
  let store = ConfigStore::load("config.ron")?;
  store.watch()?; // Spawns thread
  let cfg = store.get(); // Arc::clone, lock-free
  ```
- **Format Support**: Auto-detect RON vs TOML by extension

#### **Passive/Interactive Input Modes** (input.rs:1-350)
- **Mode A (Passive)**: `WS_EX_TRANSPARENT` — fully click-through
- **Mode B (Interactive)**: Remove `WS_EX_TRANSPARENT` + `SetTimer` for auto-revert
- **Hit-Testing**: `HitTester` with Z-ordered `InteractiveRect` list (line 79-150)
- **Transition**: Global hotkey posts `WM_GLASS_MODE_INTERACTIVE` → `SetWindowLongPtrW` to toggle `WS_EX_TRANSPARENT`

### 2.2 DirectComposition Pipeline

**Critical Insight**: HWND-based DX12 swapchains only support `DXGI_ALPHA_MODE_IGNORE` (opaque). DirectComposition bypasses this limitation via `CreateSwapChainForComposition`.

**Flow**:
1. `Compositor::new(hwnd)` → `DCompositionCreateDevice` → `CreateTargetForHwnd` → `CreateVisual` → `SetRoot` (compositor.rs:31-50)
2. `Renderer::new(visual_ptr, hwnd)` → `create_surface_unsafe(SurfaceTargetUnsafe::CompositionVisual)` (renderer.rs:145-149)
3. wgpu configures surface with `alpha_modes=[PreMultiplied]` (only available via our patched wgpu-hal)
4. `Compositor::commit()` → `device.Commit()` makes swapchain binding take effect (compositor.rs:67-73)

**Dependencies**:
- `IDCompositionVisual` pointer must outlive `wgpu::Surface` (Rust lifetime encoded via `'static` + manual safety comment)
- `WS_EX_NOREDIRECTIONBITMAP` window style suppresses GDI surface (line overlay_window.rs:4)

### 2.3 Feature Flag Strategy

| Feature | Scope | Conditional Code Locations |
|---------|-------|----------------------------|
| `test_mode` | glass-overlay + glass-poc | renderer.rs:54-93 (watermark shader), overlay_window.rs:236-241 (tooltip prefix), input.rs (force passive), diagnostics.rs (TRACE level) |
| `tracy` | glass-overlay + glass-poc | main.rs:42-54 (tracing-tracy subscriber layer) |
| `alloc-tracking` | glass-poc only | main.rs:67-68 (install tracker) |

**Pattern**: Features are opt-in, propagate via dependency features in Cargo.toml:
```toml
# glass-poc/Cargo.toml:20-21
test_mode = ["glass-overlay/test_mode"]
tracy = ["glass-overlay/tracy", "dep:tracing-tracy"]
```

---

## 3. Constraints for Minimal Standalone Example

### 3.1 Non-Negotiable Requirements

1. **Windows 10+ with DirectComposition**
   - Cannot abstract away — the entire alpha transparency mechanism depends on DComp
   - Constraint: Example will be Windows-only

2. **wgpu Premultiplied Alpha Support**
   - **Option A**: Bundle minimal wgpu-hal patch (requires including third_party/wgpu/wgpu-hal)
   - **Option B**: Use upstream wgpu and accept opaque rendering (no transparency)
   - **Recommendation**: Option A if transparency is required; Option B for true minimalism

3. **Win32 Message Pump**
   - HWND + `GetMessage` loop is unavoidable for DirectComposition target
   - Cannot use winit (it creates its own HWND with incompatible styles)

### 3.2 Eliminable Components (for Minimal Example)

| Component | Required? | Alternative for Minimal Example |
|-----------|-----------|----------------------------------|
| **glass-core** | NO | Inline `GlassError` enum directly |
| **OverlayModule trait** | NO | Hardcode single text/rect node in scene |
| **Layout system** | NO | Use fixed pixel coordinates |
| **Hot-reload config** | NO | Use hardcoded Config struct or single read |
| **Input mode switching** | NO | Stay in passive mode only |
| **Anti-cheat scanner** | NO | Skip safety checks |
| **HDR detection** | NO | Assume SDR |
| **Glyphon (text rendering)** | NO | Render rects only, or inline simple text pipeline |
| **System tray icon** | NO | Use `WM_CLOSE` → `PostQuitMessage` directly |
| **Multiple modules (clock/stats/fps)** | NO | Single demo rect/text |

### 3.3 Minimal Example Scope Proposal

**Goal**: Prove DirectComposition + wgpu + premultiplied alpha transparency in <300 LOC.

**Includes**:
- Single Rust file (no workspace, single `src/main.rs`)
- DComposition device/target/visual creation
- wgpu surface via `SurfaceTargetUnsafe::CompositionVisual`
- Minimal WGSL shader rendering single transparent quad
- Win32 HWND with `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST`
- Message loop: `WM_PAINT` → `render()`, `WM_CLOSE` → `PostQuitMessage(0)`

**Excludes**:
- Scene graph
- Modules
- Layout
- Config
- Input switching
- Text rendering
- All diagnostics/safety/HDR code

**Critical Decision**: Use patched wgpu-hal or accept opaque rendering?
- **Recommendation**: Include wgpu-hal patch in example repo as `wgpu-hal/` (not subtree, just copy 3 files), document in README why patch is needed. This preserves the core transparency demonstration.

---

## 4. Genericity Audit Strategy

### 4.1 Current Genericity Violations

#### **Hardcoded Windows Types Throughout**
- `HWND` parameters in 15+ functions (overlay_window.rs, renderer.rs, compositor.rs)
- `IDCompositionVisual` / `IDCompositionDevice` in compositor.rs (COM types, not traits)
- Win32 message constants scattered in input.rs, overlay_window.rs

#### **No Cross-Platform Abstraction Layer**
- Renderer assumes DX12 backend (line renderer.rs:140)
- No Linux/macOS equivalents for DirectComposition

#### **Tight Coupling: Scene ↔ Renderer ↔ TextEngine**
- `TextEngine::prepare` takes `&Scene` directly (text_renderer.rs:80)
- `Renderer::render` iterates scene nodes inline (renderer.rs:300-400)
- No trait abstraction for "renderable backend"

### 4.2 Audit Checklist

**For genericity audit document**:

- [ ] **Platform Abstractions**
  - Identify every HWND, COM interface, Win32 API call
  - Document which are intrinsic to DirectComposition (cannot abstract) vs. incidental (window creation, input, tray icon)
  - Propose trait boundaries: `CompositorBackend`, `InputBackend`, `SystemTrayBackend`

- [ ] **Renderer Coupling**
  - Extract `RenderBackend` trait: `fn render(&mut self, scene: &Scene) -> Result<()>`
  - Move wgpu-specific code into `WgpuDX12Backend : RenderBackend`
  - Text rendering: extract `TextBackend` trait or keep Glyphon hardcoded?

- [ ] **Module System**
  - Already generic via `OverlayModule` trait ✅
  - Downcast via `as_any_mut()` is acceptable for config updates (no better alternative without reflection)

- [ ] **Config System**
  - Hot-reload `notify` watcher is generic enough (takes `Path`)
  - RON/TOML parsing is behind `ConfigStore::load` abstraction ✅

- [ ] **Feature Flags**
  - Document canonical combinations:
    - `--no-default-features` — minimal build
    - `--features test_mode` — validation/anti-cheat testing
    - `--features tracy` — profiling
    - `--features test_mode,tracy` — NOT recommended (TRACE spam floods Tracy)

- [ ] **Error Handling**
  - `GlassError` is already an enum covering all subsystems ✅
  - Consider adding `.context()` methods for richer error chains (anyhow-style)

### 4.3 Prioritized Refactor Opportunities (Post-Audit)

**Priority 1 (High Value, Low Risk)**:
1. Extract `SystemTrayBackend` trait (overlay_window.rs:400-600) — enables headless testing
2. Document `Compositor` lifetime contract (visual must outlive surface) — add explicit lifetime parameter?
3. Split `overlay_window.rs` into `window_creation.rs` + `message_loop.rs` + `tray_icon.rs`

**Priority 2 (Medium Value, Medium Risk)**:
4. Extract `RenderBackend` trait for scene → GPU submission
5. Replace `as_any_mut()` downcast with typed config update messages (`enum ModuleConfigUpdate`)
6. Separate "passive scan" safety logic (safety.rs) from overlay code (could be standalone crate)

**Priority 3 (Low Value, High Risk)**:
7. Cross-platform compositor abstraction (DirectComposition vs. XComposite vs. macOS CALayer)
   - **Verdict**: Probably not worth it — this is a Windows overlay tool, lean into the platform

---

## 5. Architecture README Outline

**Proposed structure** (to be created as `ARCHITECTURE.md`):

```markdown
# GLASS Architecture

## 1. Overview
- Three-crate workspace (glass-core, glass-overlay, glass-poc)
- DirectComposition + wgpu DX12 + Win32 message pump
- Patched wgpu-hal for premultiplied alpha support

## 2. Crate Boundaries
- glass-core: Error type (minimal)
- glass-overlay: Core logic (compositor, renderer, scene, modules, input, config)
- glass-poc: Bootstrap harness (init, message loop)

## 3. Key Subsystems
### 3.1 DirectComposition Pipeline
- IDCompositionDevice/Target/Visual → wgpu SurfaceTargetUnsafe
- Premultiplied alpha swapchain (requires patch)

### 3.2 Retained Scene Graph
- Dirty-flag system, zero per-frame allocations
- SceneNode::Rect, SceneNode::Text

### 3.3 Module System
- OverlayModule trait: init/update/deinit lifecycle
- Built-in modules: clock, system_stats, fps_counter

### 3.4 Anchor-Based Layout
- TopLeft/TopRight/BottomLeft/BottomRight/Center/ScreenPercentage
- Automatic recalculation on WM_SIZE

### 3.5 Input Modes
- Passive (click-through) ↔ Interactive (hotkey-triggered)
- Rect-based hit-testing with Z-order

### 3.6 Hot-Reload Config
- arc-swap for lock-free reads
- notify filesystem watcher

## 4. wgpu Fork Rationale
- Upstream: DXGI_ALPHA_MODE_IGNORE (opaque)
- Patch: DXGI_ALPHA_MODE_PREMULTIPLIED (transparent)
- Subtree management: sync_wgpu.py

## 5. Feature Flags
- test_mode: Watermark, forced passthrough, TRACE logging
- tracy: Profiling via tracing-tracy
- alloc-tracking: Debug allocation instrumentation

## 6. Build & Test
- Rust 1.85+ (edition 2024)
- Windows x86_64 only
- Run: cargo run --release
- Test mode: cargo run --features test_mode

## 7. Threading Model
- Single-threaded message loop (render + input on main thread)
- Hot-reload watcher: spawns background thread (posts events to main thread)
- No Send/Sync requirements (local COM object lifetime)

## 8. Performance Characteristics
- Retained rendering: zero heap allocations in steady state
- Layout recalculation: O(n) where n = widget count (< 10)
- Hit-testing: O(n) linear scan (< 10 rects)

## 9. Safety & Anti-Cheat
- Passive scan: read-only APIs (CreateToolhelp32Snapshot, service enumeration)
- Kernel AC: blocks startup (Vanguard, Ricochet)
- User-mode AC: warns (EAC, BattlEye)

## 10. Platform Constraints
- Windows 10+ (DirectComposition)
- DX12-capable GPU
- DWM composition enabled
```

---

## 6. Planning Risks Summary

| Risk | Severity | Likelihood | Impact on Deliverables |
|------|----------|------------|------------------------|
| **wgpu patch coupling** | HIGH | Certain | Minimal example must replicate or document limitation |
| **DirectComposition expertise** | MEDIUM | Likely | Architecture README needs clear DComp lifecycle diagrams |
| **Module trait downcasting** | LOW | Possible | Genericity audit will flag `as_any_mut()` as anti-pattern |
| **Feature flag sprawl** | MEDIUM | Likely | Need testing matrix documenting valid feature combos |
| **Lifetime safety (Compositor ↔ Surface)** | MEDIUM | Possible | Manual safety comments need formalization (explicit lifetimes?) |
| **Config watcher thread panic** | LOW | Unlikely | Document thread safety (only posts Win32 messages, no shared state) |
| **Text rendering (Glyphon) coupling** | MEDIUM | Certain | Cannot easily swap text backend without significant refactor |

---

## 7. Concrete File References (Quick Lookup)

### Critical Integration Points
- **DComp Init**: `glass-overlay/src/compositor.rs:31-57`
- **wgpu Surface Creation**: `glass-overlay/src/renderer.rs:145-150`
- **Scene Dirty Iteration**: `glass-overlay/src/scene.rs:150-200`
- **Module Lifecycle**: `glass-overlay/src/modules/mod.rs:41-81`
- **Anchor Resolution**: `glass-overlay/src/layout.rs:63-87`
- **Input Mode Toggle**: `glass-overlay/src/input.rs:200-250`
- **Config Hot-Reload**: `glass-overlay/src/config.rs:150-250`
- **Message Loop**: `glass-poc/src/main.rs:244-246` → `glass-overlay/src/overlay_window.rs:400-600`

### Feature Flag Gates
- **test_mode Shader**: `glass-overlay/src/renderer.rs:54-93`
- **test_mode Tray Tooltip**: `glass-overlay/src/overlay_window.rs:236-241`
- **Tracy Layer**: `glass-poc/src/main.rs:42-54`
- **Alloc Tracking**: `glass-poc/src/main.rs:67-68`

### Windows API Usage
- **Window Styles**: `glass-overlay/src/overlay_window.rs:130-180`
- **DPI Awareness**: `glass-overlay/src/overlay_window.rs:43-51`
- **Tray Icon**: `glass-overlay/src/overlay_window.rs:400-550`
- **HDR Detection**: `glass-overlay/src/hdr.rs:35-90`
- **Anti-Cheat Scan**: `glass-overlay/src/safety.rs:150-300`

### Patched wgpu
- **Patch Location**: `third_party/wgpu/wgpu-hal/src/dx12/surface.rs` (exact line unknown, search for `DXGI_ALPHA_MODE_PREMULTIPLIED`)
- **Patch Config**: `Cargo.toml:82-85`
- **Subtree Sync**: `sync_wgpu.py` (managed via `./python sync_wgpu.py status|pull|push`)

---

## 8. Recommendations

### For Genericity Audit
1. **Accept Platform Specificity**: DirectComposition is inherently Windows-only. Don't fight this.
2. **Focus on Module System**: OverlayModule trait is the most reusable abstraction — document extension pattern.
3. **Extract Backend Traits**: `RenderBackend`, `SystemTrayBackend` (low-hanging fruit).
4. **Document Downcasting**: Explain why `as_any_mut()` is acceptable for module config updates.

### For Architecture README
1. **Lead with DirectComposition**: Most readers won't know what DComp is — prioritize pipeline diagram.
2. **Explain wgpu Patch First**: Address "why a fork?" upfront in Overview section.
3. **Include Lifetime Safety**: The Compositor → Surface lifetime contract is subtle — add ASCII diagram.
4. **Feature Flag Decision Tree**: Provide "use test_mode when..." guidance.

### For Minimal Example
1. **<300 LOC Target**: Single-file, no workspace, hardcoded rect rendering.
2. **Include wgpu-hal Patch**: Copy 3 files into example repo, document in README.
3. **No Text Rendering**: Simplify to colored rects only (removes Glyphon dependency).
4. **Inline Error Handling**: Use `.expect()` for brevity (not production-ready).
5. **Publish as Separate Repo**: `glass-minimal-dcomp-example` — easier to find via Google.

---

## Conclusion

**Architecture is production-ready but tightly coupled to Windows platform specifics.** The workspace structure is clean (3-crate hierarchy), but the core abstractions (Compositor, Renderer, Scene) are intertwined via DirectComposition's unique requirements.

**Key Insight**: The wgpu patch is the **most critical architectural decision** — it enables the entire transparency mechanism. Any documentation or minimal example **must** address this upfront, or readers will miss why the project exists.

**Planning Confidence**: **HIGH** for architecture README (mostly documentation), **MEDIUM** for genericity audit (requires design decisions on trait boundaries), **MEDIUM-HIGH** for minimal example (need to decide: patch or no patch?).

**Estimated Effort**:
- Architecture README: 4-6 hours (diagrams + API docs)
- Genericity Audit: 8-12 hours (analysis + refactor proposals)
- Minimal Example: 6-10 hours (implementation + testing + README)

**Next Steps**:
1. Review this analysis with stakeholder
2. Decide on minimal example scope (transparent or opaque?)
3. Create work breakdown for each deliverable
