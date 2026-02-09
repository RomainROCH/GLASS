---
schema: task/v1
id: task-000434
title: "Run validation campaign: Capture PresentMon/FrameView traces and document frametime, VRR, anti-cheat across CS2, LoL, Valorant (NVIDIA + AMD)"
type: chore
status: blocked
priority: medium
owner: "user"
skills: ["quality-auditor", "docs"]
depends_on: []
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Goal
Run a validation campaign to measure and document the overlay's behaviour under real-game conditions across three representative titles (CS2, League of Legends, Valorant) and two GPU vendors (NVIDIA, AMD). Collect PresentMon or NVIDIA FrameView traces, record frametime deltas, VRR behaviour, anti-cheat interaction/status, and session-length constraints; consolidate results and store trace artifacts for later analysis and audits.

## Acceptance Criteria ✅
- For each combination of Game × GPU vendor (3 games × 2 vendors = 6 runs minimum), a trace file (PresentMon OR FrameView) is produced and uploaded to the task's `Attempts / Log` section (or linked artifact storage).
- Each trace includes: frametime samples (ms), frametime delta distribution (p99/p95/median), frame rate (FPS) time series, and VRR events (mode on/off or reported VRR ticks) where applicable.
- Anti-cheat status for each run is recorded (process presence, kernel driver status, in-game/OS notifications, whether overlay injection was blocked or permitted). Specifically note EAC/Vanguard/other interactions and whether the overlay was visible/functional.
- Session length constraints are evaluated: at least one short session (≈5 minutes) and one long session (≥30 minutes) per game-vendor pair; if prolonged runs show drift or failures, note the time-to-failure and exact symptom.
- A short summary table (CSV or Markdown) is added listing: Game, GPU vendor, GPU model, driver version, Windows build, trace filename/link, frametime p50/p95/p99, average FPS, VRR observed (Y/N), anti-cheat behaviour, session length, and any notable issues.
- Validation notes and reproduction steps are added to this task so the campaign can be repeated by another engineer.

## Context & Links 🔗
- PresentMon: https://github.com/GameTechDev/PresentMon (recommended for generic captures)
- NVIDIA FrameView (Windows): https://developer.nvidia.com/frameview
- VRR / G-SYNC / FreeSync reference: OS and driver docs; ensure knowledge of how to enable/disable VRR per GPU driver.
- Anti-cheat notes: CS2 (VAC), Valorant (Vanguard), League of Legends (Riot Anti-Cheat / Riot Client interactions). Check each game's support pages for recommended test practices.
- Related tasks: `phase0-01-scaffolding.md`, `phase0-04-passthrough-window.md` (for overlay visibility tests and manual validation hooks).

## Plan / Approach 🛠️
1. Prepare two test machines (or one machine with both GPUs swapped/installed separately): one with a modern NVIDIA GPU and one with a modern AMD GPU. Record GPU model and driver version.
2. Ensure Windows updates and game clients are up-to-date. Disable unrelated overlays (Steam, GeForce Experience OSD) except the overlay under test when needed.
3. For each game (CS2, LoL, Valorant):
   - Set in-game rendering mode to commonly used settings (Fullscreen or Borderless Windowed as appropriate) and standard FPS cap (or uncapped for VRR tests).
   - Run one short session (~5 minutes) and capture a PresentMon/FrameView trace while exercising typical UI/overlay interactions (e.g., open overlay, change display modes, show/hide overlays, join a match, navigate menus).
   - Run one long session (≥30 minutes) and capture another trace to detect drift or long-running failures.
   - For VRR, run with VRR enabled and disabled (if easily switchable) and note differences.
   - Record anti-cheat observations: whether the overlay caused any warnings, was blocked, or changed behavior.
4. Naming convention for artifacts: `{game}-{gpuVendor}-{gpuModel}-{driverVersion}-{short|long}-{vrrOn|vrrOff}-{date}.zip` (include raw trace and minimal metadata JSON).
5. Upload traces to the repository's artifact location or attach to this task; add a short entry in `Attempts / Log` summarising each run and linking the artifact.

## Validation Notes 🔍 (Manual steps & checklist)
- Tools & pre-checks:
  - Install PresentMon (latest release) and/or NVIDIA FrameView; verify they produce CSV/JSON outputs on the test machine.
  - Confirm GPU drivers are set to default settings unless testing a specific VRR combination.
  - Capture Windows Event logs around test runs for any relevant driver/driver-block events or anti-cheat kernel messages.
- Capture settings:
  - PresentMon: run with `-csv -show_fps -process_name` and any other flags that capture frametime.
  - FrameView: follow vendor guidance for frametime capture; export the resulting CSV.
  - Record exact command/flags used for reproducibility.
- Run checklist per test:
  1. Note machine identifiers: GPU model, driver version, Windows build, CPU model, RAM.
  2. Launch game client and ensure the anti-cheat is active and listed (where applicable).
  3. Start trace tool, perform overlay interactions, and stop trace after required duration.
  4. Verify the trace includes frametime and frame number/timestamps; convert where needed into frametime deltas and compute p50/p95/p99.
  5. Save a minimal metadata JSON with test parameters and attach it with the trace artifact.
- Analysis notes:
  - Calculate frametime delta (ms) per frame and produce summary statistics (p50/p95/p99, stdev) and a simple time-series PNG of frametime over time for visual inspection.
  - Identify VRR indicators in trace data or driver logs (variable frame intervals matching VRR cycles) and mark as observed or not.
  - Document any anti-cheat blocks, overlay visibility issues, or crashes; include timestamps and accompanying logs.

## Attempts / Log
- **2026-02-06**: Programmatic validation completed:
  - Build: ✅ `cargo build -p glass-poc` succeeds (MSVC target, ~15.9 MB debug binary)
  - Clippy: ✅ No clippy lints (6 dead-code warnings from feature-gated alloc_tracker — expected)
  - Runtime: ✅ Process starts, creates 3440x1440 overlay window, initializes DX12 device (RTX 4070 Ti), compiles shaders, renders initial frame, enters message loop
  - Anti-cheat interaction: ⏳ Not tested (requires installing CS2/Valorant/LoL and running with overlay)
  - VRR: ⏳ Not tested
  - Multi-GPU: ⏳ Not tested (only NVIDIA GPU available — AMD testing deferred)
- **Remaining**: Manual game testing campaign requires user execution. See Plan/Approach section.

## Notes / Discoveries
- (Document any findings, flaky behaviors, or environmental quirks discovered during runs.)

## Next Steps ➡️
1. Assign an owner and run the first smoke validation (one short run on one GPU/game) to validate the workflow.
2. If the workflow needs automation (scripts to launch games/traces), create a follow-up `test-task` to implement reproducible capture scripts.
3. After campaign completion, consider creating a short guide in `docs/` describing the validation workflow and how to interpret traces.
