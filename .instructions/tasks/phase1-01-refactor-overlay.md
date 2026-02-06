---
schema: task/v1
id: task-000435
title: "Refactor overlay: Move validated PoC into `glass-overlay` and introduce OverlayWindow / Compositor / Renderer"
type: feature
status: not-started
priority: medium
owner: "unassigned"
skills: ["refactor"]
depends_on: ["task-000434"]
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Context

The PoC validated that a retained DX12/wgpu overlay approach can work across target games and GPUs (see PoC report: `.instructions/tasks/phase0-07-poc-report.md`). The project's Phase 1 plan calls for moving the validated PoC into the `glass-overlay` crate and establishing clear, testable responsibilities for overlay composition and rendering:

- See `.instructions/artefacts/glass-arch-v3_2-PLAN-artefact.md` (Phase 1 objectives, including: "Move PoC into `glass-overlay` (OverlayWindow / Compositor / Renderer)").
- Relevant PoC tasks: `.instructions/tasks/phase0-01-scaffolding.md`, `.instructions/tasks/phase0-02-wgpu-dcomp-spike.md`, `.instructions/tasks/phase0-03-triangle-render.md`, `.instructions/tasks/phase0-04-passthrough-window.md`, and `.instructions/tasks/phase0-07-poc-report.md`.

This task moves validated PoC logic (render loop, composition logic, DX12/wgpu setup scaffolding used for validation) into the `glass-overlay` crate and introduces the following well-scoped types:

- `OverlayWindow` — Window / platform integration and input/click-through behavior (thin platform shim)
- `Compositor` — Scene composition, resource management, and frame scheduling
- `Renderer` — Low-level GPU backend wrapper (wgpu/DX12 bindings) and draw submission

`glass-poc` should be kept as a thin harness that depends on `glass-overlay` and exercises the same scenarios used during PoC validation.

## Goal

Move validated PoC code into the `glass-overlay` crate, implement `OverlayWindow`, `Compositor`, and `Renderer` structs with clean, documented public APIs, and update `glass-poc` to use the new crate as a thin harness. There must be no behavioral changes beyond internal structure and code organization.

## Acceptance Criteria

1. `glass-overlay` crate exists and compiles as part of the workspace (`cargo build --workspace`).
2. Core responsibilities are implemented and exported with documented public APIs:
   - `OverlayWindow` handles platform window creation, transparency flags, and input/pass-through semantics.
   - `Compositor` accepts a scene description, schedules frames, and produces per-frame composition state for the `Renderer`.
   - `Renderer` encapsulates GPU initialization, swapchain-like presentation handling, and drawing commands.
3. `glass-poc` is converted into a thin harness that depends on `glass-overlay` and exercises the same scenarios previously validated in the PoC (rendering, click-through, HDR/DPI if applicable).
4. No behavioral changes observed by the PoC validation suite: produce equivalent or better outputs (visuals/logs/traces) for representative test cases used in Phase 0.
5. Unit tests and/or integration tests cover the public API of `OverlayWindow`, `Compositor`, and `Renderer` where practical (e.g., frame scheduling, resource lifecycle, config-driven behavior). Tests must be added for any previously untested but critical behaviors.
6. `cargo test` for the workspace passes locally and in CI.
7. The refactor leaves `glass-poc` with minimal code (thin harness); no duplication of core composition or rendering logic.
8. Update `.instructions/architecture.md` or relevant artefact with a short note describing the refactor/structure and public responsibilities of the new types.

## Plan / Approach

1. Create or verify `glass-overlay` crate structure (`Cargo.toml`, `src/lib.rs`, module layout).
2. Identify PoC source files to move (list file paths in a short sub-step) and create corresponding modules in `glass-overlay`.
3. Design minimal public interfaces for `OverlayWindow`, `Compositor`, and `Renderer` (keep interfaces small and testable). Add doc comments describing invariants and threading expectations.
4. Move code incrementally, preserving behavior at each step:
   - Move low-level GPU initialization into `Renderer` behind a minimal interface.
   - Move frame scheduling and composition into `Compositor`.
   - Move window/platform glue into `OverlayWindow`.
   - Convert the original PoC binary in `glass-poc` to call into `glass-overlay` instead of referencing moved modules.
5. Add tests for each unit where feasible and add an integration test (or harness) that runs a headless/recording run and compares a small set of pixel/screenshot outputs to known-good artifacts (from `.artifacts/poc/` if available).
6. Run the PoC validation scenarios and compare artifacts (screenshots, logs, traces) with pre-refactor artifacts to assert there are no regressions.
7. Update docs/architecture notes and add a short changelog entry in the repo describing the reorganization.
8. Open a PR that clearly states the intent: structural refactor only; include validation checklist and test instructions in the PR description.

## Validation Notes / How to Verify ✅

- Build: `cargo build --workspace` (must succeed).
- Tests: `cargo test --workspace` (must succeed).
- Behavioral parity check:
  1. Run the original PoC harness (pre-refactor commit) and the refactored `glass-poc` harness and capture screenshots or recorded frames for a representative set of games/scenarios used during Phase 0.
  2. Compare rendered output pixel-wise (or perceptual hash) and confirm no meaningful differences beyond allowed tolerances (document any deviations).
  3. Verify logs/traces show the same high-level events (frame scheduling, resource creation/destruction, present calls) without regression.
- Manual checks: launch overlay in at least two validated game/GPU combinations and confirm click-through, focus behavior, and visual composite match the PoC results.
- CI: Ensure the repository pipeline runs the build and tests; add an integration job if necessary to run an automated headless rendering comparison.

## Notes / Links

- Phase 0 PoC tasks and report: `.instructions/tasks/phase0-01-scaffolding.md`, `.instructions/tasks/phase0-02-wgpu-dcomp-spike.md`, `.instructions/tasks/phase0-03-triangle-render.md`, `.instructions/tasks/phase0-04-passthrough-window.md`, `.instructions/tasks/phase0-07-poc-report.md`.
- Architecture plan: `.instructions/artefacts/glass-arch-v3_2-PLAN-artefact.md` (see Phase 1 objectives).
- Suggested tests: unit tests for resource lifecycle and a small integration regression that compares recorded frames/screenshot artifacts to the pre-refactor set.

## Next Steps

- Assign an owner and estimate effort (hours/days).
- Optionally: create a separate test task to add more comprehensive integration/e2e validation if needed (place under `.instructions/test-tasks/`).

---

**How to validate this task (for the reviewer):** Verify `glass-overlay` crate exists and builds, `glass-poc` runs as a thin harness, unit/integration tests are present and passing, and a representative screenshot comparison shows no regressions vs. Phase 0 artifacts.
