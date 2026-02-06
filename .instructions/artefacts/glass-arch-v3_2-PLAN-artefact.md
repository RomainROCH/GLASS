# glass-arch-v3_2 PLAN artefact ✅

## Goal
Build a robust, low-overhead Windows game overlay (Glass Overlay) based on a DX12-only wgpu backend that is compatible with modern games and GPUs, prioritizing performance, transparency, and anti-cheat safety. Deliver a decisioned PoC, then iterate into a production-grade overlay with modules, config, diagnostics, and distribution.

## Assumptions
- Target platform: Windows (PerMonitorAwareV2 DPI model).
- Rendering: wgpu configured for DX12 only (DX12 feature set required).
- Primary integration path: DirectComposition with composition visuals and/or HWND fallbacks.
- Games may be sensitive to overlays; anti-cheat compatibility is a first-class concern.

## Phases (high level)
- **Phase 0 – Feasibility & PoC**: validate composition+wgpu approach, basic rendering, input/pass-through, and cross-vendor validation.
- **Phase 1 – Integration & Foundation**: move PoC into `glass-overlay`, implement retained scene graph, text rendering, HDR handling, hot-reload, DPI independence, diagnostics.
- **Phase 2 – Interaction**: passive default, interactive mode with hotkey and timed input, rect-based hit testing.
- **Phase 3 – Modules & UX**: module system + sample modules (clock, system stats, overlay FPS, config UI).
- **Phase 4 – Compatibility & Stability**: broaden game compatibility and long-duration stability testing; community beta.
- **Phase 5 – Release**: signing, release pipeline, distribution.

---

## Exact step list (concise) 🔧

**Phase 0 (PoC & validation)**
- 0.1 Project scaffolding: workspace with `glass-core`, `glass-overlay`, `glass-poc`; toolchain; DX12-only wgpu; deps; configs; update `.instructions/architecture.md`.
- 0.2 wgpu + DirectComposition feasibility spike: HWND styles; `PerMonitorAwareV2`; DComp device/target/visual; wgpu DX12 surface from composition visual; transparent clear; **decision gate**: `SurfaceTarget::Visual` works? If yes proceed; else evaluate `HWND` + `DwmExtendFrameIntoClientArea`; if both fail, stop and document why.
- 0.3 Green triangle render: 50% alpha, premultiplied, retained render; add tracing (Tracy spans).
- 0.4 Click-through + window behavior: `WM_NCHITTEST => HTTRANSPARENT`; no focus, no alt-tab, topmost; exclusive fullscreen behavior.
- 0.5 Debug allocation tracking: `GlobalAlloc` wrapper; assert zero per-frame allocations.
- 0.6 Validation campaign: CS2, LoL, Valorant; NVIDIA + AMD; session lengths; PresentMon / FrameView captures; measure frametime, VRR, anti-cheat interactions.
- 0.7 PoC report: pass/fail, collected data, screenshots; decision: proceed or stop.

**Phase 1 (Integration & foundation)**
- 1.1 Move PoC into `glass-overlay` (OverlayWindow / Compositor / Renderer).
- 1.2 Retained scene graph + text rendering (glyphon).
- 1.3 Config hot-reload (RON/TOML + notify + `ArcSwap`).
- 1.4 HDR detection + SDR fallback (`IDXGIOutput6`; scRGB; explicit fallback path).
- 1.5 Resolution / DPI independence.
- 1.6 Error handling + diagnostics dump (crash and non-crash diagnostics).

**Phase 2 (Interaction)**
- 2.1 Passive mode default (heatless overlay).
- 2.2 Interactive mode hotkey + timeout + indicator.
- 2.3 Rect-based hit testing (opt-in interactive areas).

**Phase 3 (Modules & UX)**
- 3.1 Module trait + registry.
- 3.2 Clock module.
- 3.3 System stats module.
- 3.4 Overlay-only FPS counter.
- 3.5 Config window (egui).

**Phase 4 (Compatibility & stability)**
- 4.1 Broader game compatibility sweep.
- 4.2 Long-duration stability testing and memory/handle leak audits.
- 4.3 Community beta program and feedback loop.

**Phase 5 (Release)**
- 5.1 Code signing and notarization.
- 5.2 Release build pipeline (deterministic builds, reproducible artifacts).
- 5.3 Distribution (installers, auto-update, telemetry opt-in).

---

## Risks & Mitigations ⚠️
- Anti-cheat conflicts: risk—false positives or bans. Mitigation—engage vendors early, run tests on real titles, minimize injected surface footprint, provide opt-in interactive mode.
- Driver / GPU differences: risk—behavior differs across AMD/NVIDIA/Intel. Mitigation—early validation matrix, capture PresentMon and driver info, provide fallbacks (HWND path).
- Transparency/composition failure: risk—no viable transparent wgpu surface. Mitigation—decision gate in Phase 0.2 and well-documented fallback.
- Performance / stutters: risk—overlay impacts game frametimes. Mitigation—Tracy spans, PresentMon capture, zero per-frame allocations rule, retain render architecture.
- Security & privacy: risk—shipping unsigned binary or invasive telemetry. Mitigation—privacy-first telemetry, code signing, clear opt-ins.

## Acceptance criteria (per phase, concise) ✅
- Phase 0: PoC shows a retained transparent DX12 render in target games with correct premultiplied alpha, click-through behavior works, no alt-tab focus, and validation campaign shows no immediate anti-cheat blocks for tested titles OR a documented, actionable failure mode. PoC report produced with data & screenshots.
- Phase 1: `glass-overlay` contains production-quality compositor, retained scene graph, HDR handling, hot-reload config, and diagnostics; passes local validation tests (render correctness, DPI, and HDR fallback).
- Phase 2: Interactive mode works reliably with timeout and visual indicator; rect hit-testing functions for modules.
- Phase 3: Module system is stable and ships sample modules (clock, stats, FPS, config UI).
- Phase 4: No regressions across a wider game list; long-run stability tests pass (24–72+ hour sessions as appropriate).
- Phase 5: Signed release artifacts, CI pipeline in place, and distribution channel(s) established.

---

## Validation & Metrics
- Use PresentMon / FrameView traces, Tracy spans, and per-GPU capture logs.
- Track frametime distributions, VRR artifacts, and resource usage over long sessions.
- Maintain a short checklist for each validation target (GPU vendor, game, session length, anti-cheat observation).

---

## Decision points
- Phase 0.2 verdict (SurfaceTarget::Visual success) is the primary go/no-go. If both Visual and HWND fallbacks are unacceptable, stop and reassess approach.
- PoC report (0.7): proceed only if validation shows acceptable risk profile and no blocking anti-cheat feedback.

---

**Notes:** Keep artefact brief and update `.instructions/architecture.md` with final decisions and any irreversible constraints.
