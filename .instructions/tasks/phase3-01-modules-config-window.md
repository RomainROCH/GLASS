---
schema: task/v1
id: task-000436
title: "Add Overlay modules + config window: `OverlayModule` trait, module registry, Clock/System Stats/Overlay-only FPS modules, and egui-based config window"
type: feature
status: not-started
priority: medium
owner: "unassigned"
skills: ["design", "frontend", "refactor"]
depends_on: ["task-000435"]
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Goal
Define a composable `OverlayModule` trait and a module registry; implement three core modules (Clock, System Stats, and an overlay-only FPS estimator); add a runtime configuration window implemented with `egui` that runs in a separate native window in the same process (not inside the overlay content window). Ensure metric labeling is honest (e.g., mark estimations/provenance such as "overlay-only FPS" or "system-reported") and provide simple on/off + configuration controls for each module.

## Acceptance Criteria ✅
- API & types
  - A small, documented `OverlayModule` trait exists and supports module lifecycle (init, update/tick, render-props/compose-output, serialize config) and basic metadata (id/name/description).
  - A thread-safe `ModuleRegistry` exists that lets the runtime enumerate modules, toggle them on/off, and update their per-module configuration at runtime.
- Modules implemented
  - `Clock` module: displays local time (configurable format/timezone fallback) and updates at a configurable interval.
  - `System Stats` module: reports CPU and memory usage with clear provenance labels (e.g., "system: CPU 12%", "system: memory 4.2 GiB"). Uses stable, cross-platform APIs where possible and gracefully degrades when a metric is unavailable.
  - `Overlay-only FPS` module: measures the overlay's own frame-rate (not the game) and is clearly labeled `overlay-only FPS` or `estimated overlay FPS`. It must not claim to be the game's FPS.
- Config UI
  - An `egui`-based configuration window runs as a separate native window in the same process (not composited into the overlay's transparent window). The window lists available modules with toggles and exposes module-specific configuration fields (e.g., clock format, sampling interval, filters).
  - The UI provides explanatory text near each metric that states metric provenance/limitations (e.g., "Estimated: overlay-only FPS — does not reflect game FPS").
- Hot-config and persistence
  - Module on/off state and simple configurations are hot-applicable (changes take effect immediately) and persisted to the existing config/hot-reload system (or a new `modules` config section if one doesn't exist). A small schema for serialization exists and round-trips.
- Tests & validation
  - Unit tests for `ModuleRegistry` behavior (register/unregister, enable/disable, config roundtrip) are added.
  - Integration or manual validation steps documented in `Validation Notes` below.
- Documentation
  - A short usage note is added to repo docs or `.instructions/architecture.md` describing the module model and how to open/use the config window.

## Context & Links 🔗
- Phase 1 refactor moved overlay responsibilities into `glass-overlay`: `.instructions/tasks/phase1-01-refactor-overlay.md` (depends on this task).
- Related Phase tasks: `.instructions/tasks/phase1-03-config-hotreload.md` for hot reload / config persistence patterns.
- Existing UX/architecture notes: `.instructions/architecture.md`.

## Plan / Approach 🛠️
1. Design API
   - Draft a minimal `OverlayModule` trait (Rust): lifecycle methods (e.g., `fn init(&mut self, ctx: &Context)`; `fn update(&mut self, delta: Duration)`; `fn render(&self, ctx: &RenderContext)`), metadata (id, name, description), and config serialization (serde-friendly types).
   - Define `ModuleRegistry`: register, enumerate, enable/disable, get/set config, and change notifications (observer pattern or channels).
2. Implement core modules
   - `Clock`: read system time, format per config, tick at configurable interval; add minimal tests for formatting/interval behavior.
   - `System Stats`: implement using platform-appropriate libs/APIs (e.g., `sysinfo` crate or `psutil` equivalents), include provenance labels and graceful fallback when metrics cannot be read.
   - `Overlay-only FPS`: implement an estimator that counts overlay render ticks and reports a running FPS averaged over a small window; mark the metric as `overlay-only`/`estimated` in metadata/UI.
3. Config window (egui)
   - Add `egui` as a dependency (or re-use existing UI dependency if already present) and implement a small native `egui` window in-process (separate from the overlay window). The window should be opened via a hotkey or tray/menu action and not interfere with overlay transparency/click-through behavior.
   - UI lists all registered modules, shows toggle + a short description + provenance note for each metric, and shows module-specific config fields inline or in a modal.
   - Ensure changes are applied immediately and persisted via the project's config system (or a new `modules` config path).
4. Testing & validation
   - Add unit tests for `ModuleRegistry` and module config serialization.
   - Add a manual integration validation section (see Validation Notes below).
5. Docs & small examples
   - Add brief documentation / README snippet describing how to open the config window, toggle modules, and understand metric provenance labels.

## Validation Notes 🔍
Manual checks (quick):
1. Build & run the overlay app locally.
2. Open the config window (hotkey/menu). Confirm it is a native window separate from the overlay and that it does not affect overlay click-through behavior when closed.
3. Toggle `Clock` on/off and change format/timezone. Verify time updates and format reflects config immediately.
4. Toggle `System Stats` and confirm CPU/memory stats appear; verify labels include provenance (e.g., `system:` prefix) and that metrics gracefully show `N/A` if the platform cannot provide values.
5. Toggle `Overlay-only FPS` on and observe reported value while interacting with the overlay. Confirm the UI label clearly states `overlay-only FPS` or `Estimated` to avoid misleading users.
6. Persist config: restart the app and confirm module enable states and basic configs survive restart.
7. Run unit tests for `ModuleRegistry` and module config roundtrips.

Programmatic/integration checks (optional):
- Add a small automated test that spins the overlay headless with a configurable frame tick rate and asserts the `Overlay-only FPS` estimator reports a value within a tolerance of the expected tick frequency.

## Notes / Questions ❓
- Which UI crate do we prefer for the config window if `egui` is not already in the project? The task assumes `egui` (desktop native window), but the team may prefer another lightweight option.
- Confirm any constraints about exposing system metrics (privacy or permission concerns) and whether additional opt-in is required before reading system stats.
- Decide if modules should be discoverable via a plugin directory or hard-registered at compile time (start with compile-time registration and add plugin extensibility later if desired).

## Next Steps ➡️
1. Assign an owner and confirm `egui` dependency acceptance.
2. Implement API + `ModuleRegistry` and add `Clock` module as the first incremental change.
3. Implement config window and `System Stats` and `Overlay-only FPS` modules.
4. Add tests and documentation, then open a PR with clear validation instructions for reviewers.

---

**How to validate this task (for the reviewer):**
- Build and run the app; open the config window and verify module toggles and config fields work and persist; confirm honest labeling for all metrics and that the `overlay-only FPS` metric is correctly labeled as an estimate of the overlay's own frames.
