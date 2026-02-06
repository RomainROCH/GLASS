---
schema: task/v1
id: task-000434
title: "Add debug allocation tracking: `GlobalAlloc` wrapper with per-frame zero-allocation assertion"
type: chore
status: archived
priority: medium
owner: "executive2"
skills: ["debug", "quality-auditor"]
depends_on: []
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Context

This task implements a debug-only allocation tracking facility for the overlay runtime. The goal is to make allocation regressions obvious during development by asserting that there are zero allocations per frame after initialization, and by emitting actionable diagnostics when the rule is violated.

See the plan artefact: `.instructions/artefacts/glass-arch-v3_2-PLAN-artefact.md` (item: "0.5 Debug allocation tracking: `GlobalAlloc` wrapper; assert zero per-frame allocations").

Related phase files:
- `.instructions/tasks/phase0-01-scaffolding.md`
- `.instructions/tasks/phase0-02-wgpu-dcomp-spike.md`
- `.instructions/tasks/phase0-03-triangle-render.md`
- `.instructions/tasks/phase0-04-passthrough-window.md`

## Goal

Add a debug-only `GlobalAlloc` wrapper that counts allocations, reset counts per-frame, asserts zero allocations per-frame after initialization, and when violated emits logs and a Tracy event/span with sufficient context to find the allocation site.

## Acceptance Criteria ✅

1. A debug-only global allocator wrapper exists (enabled via build cfg or feature flag such as `alloc-tracking` or `debug_alloc`).
2. The wrapper increments an allocation counter for each allocation (and tracks frees as needed to avoid drift).
3. The counter is reset each frame (or per-frame epoch) and checked after the first full frame following initialization.
4. If per-frame allocation count > 0 after init, the system:
   - Emits a clear log message with allocation count and a short stack trace (where feasible), and
   - Emits a Tracy message/zone with the allocation count and stack-sample/tag to make profiling immediately visible.
5. A small, deterministic validation harness exists (run only in debug builds or via a test runner) to intentionally allocate and confirm the detection/logging behavior.
6. Instrumentation is off in release builds (no measurable overhead) and guarded behind an explicit debug feature or build flag.

## Plan / Approach 🔧

1. Add a `debug_alloc` feature or gate behind `cfg(debug_assertions)` to avoid release cost.
2. Implement a `GlobalAlloc` wrapper that forwards to the real allocator while incrementing an atomic counter on allocation (consider thread-local counters to reduce contention and avoid false positives from unrelated threads).
3. Add a per-frame epoch counter or reset point in the main loop (render loop) to zero the per-frame counters and perform the assertion/validation after init-complete.
4. On violation, log via existing logging API and submit a Tracy zone or message (use the project’s Tracy integration) with details and a stack sample if available.
5. Add a debug-only test/harness to exercise detection (force an allocation during a frame and verify logs/Tracy event).
6. Document the feature and how to enable/disable it.

## Validation Notes ✅

- Manual validation steps:
  1. Build and run a debug build with allocation tracking enabled.
  2. Let the app initialize, then run several frames; confirm per-frame allocation counter is zero.
  3. Introduce a small intentional allocation in the frame path (temporary test) and confirm:
     - The log message appears with allocation count and short stack trace.
     - A Tracy event/zone is visible in the profiler with the same information.
- Automation: add an integration/debug-only test that runs the loop for N frames, injects a controlled allocation and asserts the detection.

## Notes / Gotchas

- Some allocations may legitimately occur from OS or third-party libraries; the check should be scoped to the overlay's allocation sites where practical (or make it opt-in per module).
- If using thread-local counters, ensure counters are aggregated correctly each frame and that worker threads are excluded or accounted for to avoid false positives.
- Be careful around initialization and resource loading phases—only start asserting after a clear "init complete" point.

## Next Steps

- Decide whether to gate using `cfg(debug_assertions)` or an explicit feature flag (recommend explicit feature flag + debug guard).
- Implement the wrapper and add the per-frame assertion.
- Add the validation harness and document usage.

---

**Suggested Adjacent Tasks:**
- Add a test task for the validation harness (`.instructions/test-tasks/`) to ensure CI can run the debug harness locally.
- Create a note in `project.memory.md` about common allocation sources and gotchas for future debugging.


