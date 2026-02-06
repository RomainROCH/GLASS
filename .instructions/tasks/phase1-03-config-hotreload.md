---
schema: task/v1
id: task-000001
title: "Add hot-reloadable config: RON/TOML + notify + ArcSwap (phase1-03)"
type: feature
status: not-started
priority: high
owner: "<dev-handle>"
skills: ["feature-creator", "logging-observability", "system-editor"]
depends_on: []
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Context

Phase 1 requires a robust, low-latency configuration system so overlays can be tuned without restarts. The architecture artefact already lists this as a Phase 1 item: **1.3 Config hot-reload (RON/TOML + notify + ArcSwap)** (see `.instructions/artefacts/glass-arch-v3_2-PLAN-artefact.md`).

This task implements the runtime config for overlay position/size/opacity/colors and adds hot-reload behavior:

- Config formats: support `RON` (preferred for Rust-native types) and `TOML` as a fallback/user-facing format.
- Watcher: use the `notify` crate to watch config file(s) and trigger reloads on write/rename.
- Lock-free reads: expose config via `ArcSwap` (or equivalent) so render/logic can read config without locks and without allocations per frame.
- Explicit reload logging: trace/log each reload attempt/result with before/after diffs and error details when parsing fails.

Related files / tasks:
- `.instructions/artefacts/glass-arch-v3_2-PLAN-artefact.md` (Phase 1 roadmap)
- `.instructions/tasks/phase0-01-scaffolding.md` (project/workspace scaffolding)
- `.instructions/tasks/phase0-03-triangle-render.md` (render acceptance criteria)

## Acceptance Criteria ✅

1. The overlay runtime reads a well-documented config file at startup (default path: `config.ron` or `config.toml`).
2. The config defines the overlay's position (x,y or anchor), size (width/height or scale), opacity (0.0–1.0), and primary/secondary colors (hex/RGBA) with reasonable defaults and validation.
3. Config parsing supports at least RON and TOML (library picks either by extension or explicit `format:` field) and provides clear parse errors.
4. Config values are exposed to readers via a globally accessible `ArcSwap<Config>` (or equivalent) allowing lock-free, allocation-free reads from hot code paths (render loop, input handlers).
5. Modifying and saving the config file triggers a hot reload via `notify` within ~100–500ms of the FS change on dev machines and applies new config state atomically.
6. Successful reloads log a concise message with a small delta/summary of what changed; failed reloads log error details and keep the last-known-good config active.
7. Add unit tests for parsing both formats, a small integration test for file-watcher-triggered reloads (running in temp dir), and CI checks for config validation.
8. Document config format and default location in README / `docs/` (short example included).

## Plan / Approach 🔧

1. Define a `Config` Rust struct that covers:
   - `position: { x: f32, y: f32 }` or `anchor: enum` (document preferred API)
   - `size: { width: f32, height: f32 }` or `scale: f32`
   - `opacity: f32` (0.0..=1.0)
   - `colors: { primary: String, secondary: Option<String> }` (hex or RGBA)
   - `metadata: { last_modified: Option<SystemTime> }` (for diagnostics)
2. Add `serde` + `ron` and `toml` deserialization support; choose parsing by file extension or explicit `format` field.
3. Add a `ConfigStore` component:
   - Holds an `ArcSwap<Config>` for lock-free readers.
   - Exposes `load(path) -> Result<Config>` and `apply(Config)` which swaps the Arc.
   - Validates config on parse; rejects invalid values (with helpful errors) without applying them.
4. Implement a `notify`-based watcher that:
   - Watches one or more configured paths.
   - Debounces events (short window ~100ms) and calls `load` on write/rename events.
   - On success, `apply` the new config and emit a reload log with a small field-level diff.
   - On failure, log parse/error details and keep the previous config.
5. Wire the `ArcSwap` reads into the overlay render path with short-lived snapshots (zero allocations) and test that reads are allocation-free in the hot path.
6. Instrument reloads with `tracing`/`log`: emitted events must include: time, source path, parse result, changed fields (summary), and which module applied the change.
7. Add docs: config example file (RON + TOML), a short README section, and developer notes on hot-reload behavior and troubleshooting.

## Validation Notes ✅

Manual validation:
1. Run the overlay with the example config; confirm values (position, size, opacity, colors) are applied on startup.
2. Edit and save the config file; confirm the watcher logs a reload event and the overlay updates visibly without a restart.
3. Intentionally insert a parse error; confirm the reload logs an error and the overlay continues using the previous config.
4. Confirm reload logging contains a small summary (e.g., `opacity: 0.8 -> 0.65`, `position.x: 100 -> 120`) and has a trace/span linking parsing → apply.

Automated tests / CI:
- Unit tests:
  - Roundtrip parse/serialize for RON and TOML example configs.
  - Validation tests (reject out-of-range opacity, malformed color strings, negative sizes if not supported).
- Integration tests:
  - Create temp config file, start watcher, write new config, assert `ArcSwap` was swapped and callback observed.
  - Simulate concurrent readers (mimic render loop) while applying config and assert no panics/data races.
- Bench micro test to confirm reads from `ArcSwap` in the hot path are allocation-free (or acceptably low overhead).

## Notes / Questions ❓

1. Which format should be the canonical example? (RON is Rust-native and supports richer types; TOML is more user-friendly.)
2. Default config filename and search rules (only `config.ron`/`config.toml` in CWD, or accept `~/.glass/config.*`?): please confirm preference.
3. Owner assignment: who should I set as the `owner` for this task? (Currently `"<dev-handle>"`.)

## Suggested Adjacent Work

- Add a test-specific task to expand integration tests to include Windows-specific filesystem notification edge cases.
- Add a small config UI or CLI to dump current effective config and to force a reload for debug builds.

---

**Validation:** To mark done, provide a README snippet demonstrating the example config, pass the unit & integration tests, and include a short recorded demo (gif or short video) showing hot-reload in action.
