---
schema: task/v1
id: task-000436
title: "Input modes: Implement passive (Mode A) and interactive (Mode B) input behavior with rect-based hit-testing"
type: feature
status: not-started
priority: medium
owner: "unassigned"
skills: ["feature-creator", "refactor"]
depends_on: ["task-000435"]
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Context

The overlay must support two complementary input modes to balance uninterrupted gameplay (click-through) with occasional user interaction:

- **Mode A (Passive)** — default runtime behavior; the overlay is fully click-through and does not receive mouse input (Windows `HTTRANSPARENT` / WS_EX_TRANSPARENT style semantics).
- **Mode B (Interactive via Hotkey)** — the user triggers an interactive mode with a global hotkey (`RegisterHotKey`) or similar, enabling mouse interaction for a configurable timeout and showing a visual indicator while interactive.

Relevant artifacts and decisions:

- PoC/documentation on passthrough and window flags: `.instructions/tasks/phase0-04-passthrough-window.md`
- Phase 1 overlay refactor that introduced `OverlayWindow`/compositor responsibilities: `.instructions/tasks/phase1-01-refactor-overlay.md`
- This task implements precise, rect-based hit testing for interactive UI nodes so only defined interactive rectangles will accept input while other areas remain pass-through.

## Goal

Implement the two input modes and rect-based hit-testing for interactive nodes:

1. Default Mode A (Passive): overlay windows are click-through using platform-appropriate flags (`HTTRANSPARENT` / equivalent).
2. Mode B (Interactive): a registered hotkey toggles a short-lived interactive state (default timeout: 4000 ms) where designated interactive rects accept mouse input. A visual indicator (e.g., a subtle border / translucent overlay or HUD element) is shown while interactive.
3. Rect-based hit-testing: interactive UI nodes must expose rectangle bounds for hit-testing; only these areas accept mouse events when in interactive mode. Hit-testing should respect stacking/Z order and return the nearest eligible interactive node.
4. Behavior must be configurable, testable, and documented.

## Acceptance Criteria

1. **Mode A (Passive):** On normal startup the overlay is click-through — mouse events are delivered to underlying windows and not to the overlay (verify via `HTTRANSPARENT` / window style on Windows).
2. **Mode B (Interactive):** Pressing the configured global hotkey puts the overlay into interactive mode for the configured timeout (default 4000 ms). While interactive:
   - The overlay accepts mouse input on interactive rects only.
   - A visual indicator is displayed for the duration.
   - Interactivity ends automatically after timeout and the overlay returns to Mode A.
3. **Rect-based Hit Testing:** Interactive regions are defined by rectangles; hit-testing returns the correct interactive node considering Z-order, and clicks in non-interactive areas are ignored and pass through to underlying windows.
4. **Configurability:** Hotkey, timeout, and visual indicator style are configurable (defaults provided).
5. **Tests:** Unit tests validate hit-test logic (rect containment, Z-order selection, timeout behavior). Integration or manual validation steps for hotkey registration and UI indicator are documented.
6. **No regressions:** Implementation does not break existing window transparency, rendering, or other overlay behaviors validated by Phase 0/1 validation steps.

## Plan / Approach

1. Design a small input-mode subsystem inside `OverlayWindow` (or a new `InputMode` component) that tracks current mode, timer, and hotkey state.
2. Implement platform glue for mode switching:
   - Windows: set/clear `HTTRANSPARENT` / WS_EX flags or use native hit-test handling when interactive.
   - Implement global hotkey registration (`RegisterHotKey`) and safe lifetime/cleanup; expose a cross-platform abstraction for other platforms (no-op or platform-specific alternative) so code is testable.
3. Add a lightweight `HitTest` abstraction that accepts a list of interactive rects (with z-order) and returns the topmost hit node for a given point.
4. Integrate visual indicator rendering into the compositor or overlay UI layer; keep styling pluggable so tests can assert indicator presence without relying on pixel-perfect compares.
5. Add unit tests for `HitTest` (rect containment, overlapping rects, z-order resolution) and for `InputMode` timer behavior and transitions.
6. Add an integration or manual test scenario in `glass-poc` that demonstrates default passthrough, hotkey activation, indicator, clickable test nodes, and timeout reversion.
7. Update docs/architecture and a short usage note in README describing hotkey, timeout, and how to add interactive rects.

## Validation Notes / How to Verify ✅

- Build: `cargo build --workspace` (must succeed).
- Unit tests: add and run tests for `HitTest` and `InputMode` (e.g., `cargo test -p glass-overlay`), asserting deterministic logic (no flakiness around timers; use mockable clocks where possible).
- Manual validation steps:
  1. Launch the overlay harness (e.g., `glass-poc` or dev harness) with a simple interactive scene: draw a few rectangles marked as interactive.
  2. Confirm default behavior: clicking through the overlay interacts with underlying application windows and not the overlay.
  3. Press the configured hotkey: observe the visual indicator and verify that clicking inside an interactive rectangle triggers an overlay event/handler; clicking outside interactive rectangles still passes through.
  4. Wait for timeout (or simulate fast-forwarded/mock clock): verify overlay returns to passive click-through and indicator disappears.
  5. Test overlapping interactive rects to verify the topmost rect receives the click.
  6. Test rapid re-triggering of hotkey and ensure timer resets/behaves as defined by spec.
- Automation notes: Consider adding a headless or harness-based integration that records callbacks fired by overlay click handlers to assert correct hit-test routing.

## Notes / Links

- Pass-through writeups and PoC: `.instructions/tasks/phase0-04-passthrough-window.md`.
- Overlay refactor and `OverlayWindow` responsibilities: `.instructions/tasks/phase1-01-refactor-overlay.md`.
- Suggested test task (optional): add a test task under `.instructions/test-tasks/` to add more e2e/integration coverage for the hotkey and interactive flow.

---

**How to validate this task (for the reviewer):**
- Run unit tests for `HitTest` and `InputMode` and confirm deterministic results.
- Follow the manual validation steps above using `glass-poc` or a small dev harness scene and confirm Mode A default passthrough, hotkey-driven Mode B activation, visible indicator, correct rect-based hit-testing, and automatic timeout reversion.

