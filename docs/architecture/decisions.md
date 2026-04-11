# Architecture Decision Records

This document records the key architectural decisions made for GLASS, a Windows overlay framework built on DirectComposition and wgpu/DX12. Each decision captures the context, rationale, alternatives considered, and consequences.

These decisions were made through architectural analysis of the full design space, studying how existing overlay systems (GeForce Experience, Discord, Steam, RTSS, Overwolf) solve the same problems, and evaluating what tradeoffs are appropriate for a lightweight, generic overlay framework.

---

### ADR-001: Windows-only — No Platform Abstraction

**Status**: Accepted  
**Date**: 2026-04-11  

**Context**: Should GLASS abstract the OS layer behind a trait to support macOS and Linux in the future?

**Decision**: No. GLASS targets Windows exclusively. This is a deliberate feature, not a limitation.

**Rationale**: DirectComposition — the compositor API that GLASS depends on for transparent, always-on-top, click-through rendering — has no cross-platform equivalent. Each OS has fundamentally different constraints:

- **macOS**: Core Animation + Metal. Different compositing model, no straightforward equivalent to DComp's click-through always-on-top behavior above fullscreen apps.
- **Linux/Wayland**: Wayland's security model explicitly forbids unmanaged overlays. A Wayland overlay would require compositor-specific protocols (layer-shell), and X11 is fading.
- **Abstraction cost**: A platform trait would be either too fine-grained (exposing platform concepts that don't generalize) or too coarse (losing capabilities that make each platform's overlay useful). Overwolf maintains dedicated teams per OS for this reason.

The target market is PC gamers — over 96% on Windows. The ROI of porting is near zero for an indie framework.

**Alternatives considered**:
- *Platform trait with OS-specific backends*: Rejected. The APIs share almost no surface area. The trait would be a fiction.
- *macOS-only secondary target*: Rejected. No demand, and Core Animation + Metal is a separate framework.

**Consequences**: `compositor.rs`, `overlay_window.rs`, and `renderer.rs` use Win32 and DirectComposition APIs directly. There is no `Platform` trait or conditional compilation for other OSes. If someone wants to port GLASS, the extension points are clear — replace those three files — but no abstraction is built preemptively.

---

### ADR-002: External Window — No DLL Injection

**Status**: Accepted  
**Date**: 2026-04-11  

**Context**: Overlays can be implemented two ways: as an external transparent window composited by the OS (what GLASS does), or by injecting a DLL into the game process and hooking the graphics API's Present call to render directly into the game's swapchain (what RTSS and classic Discord overlay did). Should GLASS support in-process rendering?

**Decision**: No. GLASS is always an external window process. It never injects into a game.

**Rationale**: External and in-process overlays share almost no architecture:

| Aspect | External (GLASS) | In-process (injection) |
|---|---|---|
| Window | Transparent HWND via DComp | No window — renders into game's swapchain |
| Rendering | Own wgpu device + swapchain | Game's D3D device, hooked Present |
| Compositing | OS compositor (DWM) | Directly in game's render pipeline |
| Anti-cheat | Safe by design | Triggers anti-cheat detection |

Trying to support both would mean building two frameworks under one name. More importantly:

- **Discord rebuilt their overlay in 2025** from injection to external window, precisely because injection conflicts with kernel-level anti-cheats (Vanguard, EAC, BattlEye).
- **Fullscreen exclusive is dying**. Nearly all modern games run borderless windowed. Windows fullscreen optimizations silently convert even "exclusive fullscreen" to borderless in most cases.

**Alternatives considered**:
- *Injection mode as opt-in*: Rejected. The codepaths share nothing. It would be a separate project.
- *Hybrid: external window + optional injection for legacy fullscreen*: Rejected. Injection carries anti-cheat risk that contradicts GLASS's safety-by-design stance.

**Consequences**: GLASS is anti-cheat safe — it never touches another process's memory or graphics device. It works in borderless windowed mode (the vast majority of modern games). It does **not** work in true exclusive fullscreen (increasingly rare). GLASS is agnostic about where data comes from: a consumer can inject DLLs to read game memory and feed data to GLASS via IPC, but that is the consumer's responsibility, not the framework's.

---

### ADR-003: wgpu over Raw Direct3D

**Status**: Accepted  
**Date**: 2026-04-11  

**Context**: GLASS needs hardware-accelerated rendering for its overlay. Should it use wgpu (a Rust graphics abstraction over D3D12/Vulkan/Metal) or call the Direct3D 12 API directly?

**Decision**: Use wgpu with the DX12 backend.

**Rationale**: Raw D3D12 makes sense for in-process hooking where you must interoperate with the game's device and swapchain — but ADR-002 rules that out. For an external overlay framework, wgpu provides the right abstraction level:

- Hardware-accelerated 2D and 3D rendering without being locked to a single graphics API at the code level.
- A potential path to a Vulkan backend if needed in the future.
- A well-maintained Rust-native API with an active ecosystem (shader tooling via naga, etc.).

The tradeoff is a dependency on a patched wgpu fork — wgpu upstream does not support `DXGI_ALPHA_MODE_PREMULTIPLIED` on composition swapchains, which is required for transparent overlays (see ADR-006).

**Alternatives considered**:
- *Raw D3D12*: Rejected. Enormous API surface, manual resource lifetime management, no benefit given external-window architecture.
- *Raw D3D11*: Rejected. Simpler than D3D12 but still vendor-locked with no path to Vulkan.
- *Direct2D*: Rejected. 2D-only, tied to Windows, and would limit future rendering capabilities.

**Consequences**: GLASS depends on a patched wgpu fork vendored in `third_party/wgpu/`. The patches touch `wgpu-core`, `wgpu-hal`, and `wgpu-types` to enable premultiplied alpha on composition swapchains. Upstream wgpu updates require careful rebasing of these patches. This is currently the primary source of technical debt.

---

### ADR-004: No DataBus — Callback Injection per Module

**Status**: Accepted  
**Date**: 2026-04-11  

**Context**: Should GLASS provide a centralized data bus (e.g., `trait DataProvider { fn get(&self, key: &str) -> Option<DataValue>; }`) for modules to consume external data, or should each module manage its own data source?

**Decision**: Each module manages its own data source via callback injection. There is no framework-level data bus.

**Rationale**: A centralized DataBus is over-engineering for a generic overlay framework:

- It introduces string-typed or enum-typed keys, losing Rust's strong typing.
- It couples all modules to an abstraction that only makes sense for monitoring-style overlays.
- Many modules need no external data at all. `ClockModule` reads the system clock. `FpsCounterModule` counts its own frames. A notification module renders messages it receives.

The callback injection pattern (`SystemStatsModule::set_temp_source()`) keeps the type system intact: each module declares exactly what external data it accepts, with a concrete closure type. The consumer's `main.rs` does the wiring.

If a consumer application has many data sources (hardware sensors, game state, network stats), it can build its own data layer and inject closures into GLASS modules. GLASS should not impose how data circulates — only how it renders.

**Alternatives considered**:
- *Generic `DataProvider` trait*: Rejected. Stringly-typed or enum-typed keys lose compile-time safety. Every module would need to parse and validate values at runtime.
- *Event/message bus*: Rejected. Adds pub/sub complexity, ordering concerns, and allocation pressure — all unnecessary for an overlay that updates a few text nodes every 1–2 seconds.

**Consequences**: The `SystemStatsModule::set_temp_source(callback)` pattern is the recommended approach. GLASS modules declare typed callback setters for any external data they need. Consumer applications (like Pulse) can build their own data bus if they have 8+ data sources, and inject closures that bridge their bus to GLASS modules.

---

### ADR-005: In-Process Plugins Only via OverlayModule

**Status**: Accepted  
**Date**: 2026-04-11  

**Context**: Should GLASS provide an out-of-process plugin system where third-party plugins run in separate processes and communicate via IPC?

**Decision**: No. All plugins are in-process Rust crates implementing the `OverlayModule` trait. Out-of-process communication is the consumer's responsibility.

**Rationale**: An out-of-process plugin system requires:

- An IPC protocol (named pipes, shared memory, or sockets)
- A serialization format for scene graph commands
- Latency management for real-time rendering
- Plugin discovery and lifecycle management
- Error isolation and crash recovery

This is essentially building a mini-Overwolf. For a lightweight overlay framework, it is disproportionate.

The pragmatic alternative works: if a consumer needs an external plugin, they create an `IpcBridgeModule` that implements `OverlayModule`, receives data via named pipe (or whatever transport they prefer), and creates scene nodes. GLASS does not need to know about IPC — the bridge module is just another `OverlayModule` from the framework's perspective.

**Alternatives considered**:
- *Named-pipe plugin protocol*: Rejected. Defines a wire format that all plugins must implement. Limits expressiveness to what the protocol supports. Adds latency.
- *Shared-memory scene graph*: Rejected. Complex synchronization, platform-specific, and still requires a discovery/lifecycle protocol.
- *Dynamic library (`.dll`) loading*: Rejected. ABI instability in Rust, unsafe FFI boundary, version compatibility issues.

**Consequences**: All modules must be Rust crates compiled with the application. Hot-reload requires recompilation. There is no language-agnostic plugin API. A panicking module crashes the entire application (no isolation). These are acceptable tradeoffs for a lightweight overlay framework where the consumer controls the full module set.

---

### ADR-006: DirectComposition for Transparency

**Status**: Accepted  
**Date**: 2026-04-11  

**Context**: How should GLASS achieve per-pixel alpha transparency for its overlay window? A transparent overlay must render colored pixels with partial opacity over the desktop and game content beneath it.

**Decision**: Use DirectComposition with `DXGI_ALPHA_MODE_PREMULTIPLIED` swapchains.

**Rationale**: The alternatives were evaluated systematically:

| Technique | Result |
|---|---|
| HWND-based DX12 swapchain | Only supports `alpha_modes=[Opaque]`. No transparency. |
| Layered windows (`UpdateLayeredWindow`) | Legacy API. Requires CPU-side bitmap composition. Incompatible with hardware-accelerated rendering. |
| DWM thumbnails | Mirrors existing windows. Cannot render arbitrary content. |
| Direct2D | 2D-only. Locked to Windows. Cannot leverage wgpu. |
| **DirectComposition** | `CreateSwapChainForComposition` supports `DXGI_ALPHA_MODE_PREMULTIPLIED`. True per-pixel alpha with hardware acceleration. |

DirectComposition is the modern Windows compositor API. The rendering flow is:

1. Create a `DCompDevice` (via `DCompositionCreateDevice`).
2. Create a composition target bound to the overlay HWND.
3. Create a visual and attach it to the target.
4. wgpu creates a swapchain via `CreateSwapChainForComposition` and binds it to the visual.
5. After wgpu surface configuration, commit the DComp device to make the visual tree live.

All overlay content uses premultiplied alpha: RGB values are pre-multiplied by their alpha channel before storage. This is required by DXGI composition swapchains.

**Alternatives considered**: See table above.

**Consequences**: Requires the patched wgpu fork (ADR-003) to expose `DXGI_ALPHA_MODE_PREMULTIPLIED` through wgpu's surface configuration. The `Color` type in `scene.rs` uses premultiplied alpha throughout. Module authors must be aware that colors are premultiplied (e.g., 50% opacity red is `Color { r: 0.5, g: 0.0, b: 0.0, a: 0.5 }`, not `Color { r: 1.0, g: 0.0, b: 0.0, a: 0.5 }`).

---

### ADR-007: Retained Scene Graph with Dirty Tracking

**Status**: Accepted  
**Date**: 2026-04-11  

**Context**: Should GLASS use an immediate-mode rendering approach (rebuild the entire UI every frame, à la Dear ImGui) or a retained scene graph (create nodes once, update selectively)?

**Decision**: Retained scene graph with explicit dirty flags.

**Rationale**: An overlay's content is mostly static. A typical monitoring overlay updates text every 1–2 seconds. Between updates, every frame is identical. The two approaches differ dramatically in steady-state cost:

| Aspect | Immediate mode | Retained + dirty tracking |
|---|---|---|
| Steady-state CPU | Rebuild layout + generate draw calls every frame | Near zero — skip unchanged nodes |
| Steady-state GPU | Re-upload all buffers every frame | Zero uploads when nothing changed |
| Heap allocations | Per-frame string formatting, vertex generation | Zero in steady state |
| Complexity for module authors | Lower — just emit draw calls | Slightly higher — manage node IDs and updates |

For an always-on overlay that must minimize its impact on game performance, retained mode with dirty tracking is the correct tradeoff. The overlay should consume effectively zero GPU time when nothing changes.

**Alternatives considered**:
- *Immediate mode (Dear ImGui style)*: Rejected. Simpler API, but per-frame cost is unacceptable for an always-on overlay. Even "cheap" immediate-mode rendering adds measurable GPU overhead when running at 144+ Hz alongside a game.
- *Retained mode without dirty tracking*: Rejected. Still re-renders the full scene every frame. Misses the key optimization.
- *Reactive/signals model (like Leptos/Dioxus)*: Rejected. Adds a reactive runtime, virtual DOM diffing, and allocation overhead. Overkill for a scene graph with 10–50 nodes.

**Consequences**: Scene nodes (in `scene.rs`) are created once via `Scene::add_*` and identified by `NodeId`. Updates go through `Scene::update_*` methods which mark nodes dirty. The renderer only processes dirty nodes on each frame. In steady state, there are zero heap allocations and zero GPU buffer uploads. Module authors must manage `NodeId` handles and call update methods explicitly rather than re-declaring their UI each frame. The `OverlayModule::update()` method receives a `&mut Scene` and a delta time, and is expected to update only what changed.
