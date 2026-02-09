---
schema: task/v1
id: task-000436
title: "Beta testing: broader game compatibility matrix, soak tests, memory & sleep/wake validation"
type: chore
status: not-started
priority: high
owner: "unassigned"
skills: ["testing-dotnet-unit", "docs", "planning-feature"]
depends_on: ["phase3-01-modules-config-window", "phase3-02-selfcheck-anticheat"]
next_tasks: ["phase5-01-signing-release"]
created: "2026-02-06"
updated: "2026-02-06"
---

## Goal
Run a focused beta testing campaign to validate broader game compatibility and runtime stability. Deliver a compatibility matrix covering target game titles and hardware, complete 8-hour soak tests on representative systems (including memory/leak monitoring), verify behavior across sleep/wake cycles, distribute a community beta build to collect issues, and produce a prioritized issue backlog and validation notes.

## Acceptance Criteria ✅
- Compatibility matrix populated for at least 20 representative game titles across a mix of engines (DirectX 11/12, Vulkan) and genres (FPS, RPG, simulators) and mapped to test platforms (GPU vendor, OS build).
- 3 representative systems (low/medium/high spec) complete unattended 8-hour soak tests with automated telemetry collection and logs captured.
- Memory and resource usage tracked during soak tests with analysis showing no unbounded growth (or a documented, reproducible leak with reproduction steps and stack traces).
- Sleep/wake test cases executed on at least two Windows versions (Win10, Win11): overlay resumes without crashes/visual corruption and recovers input/hotkeys reliably.
- Community beta distribution executed (invite list / opt-in link) and at least 30 unique beta sessions recorded with issue reports collected into the repo's issue tracker or a central triage board.
- A reproducible issue backlog is created and triaged with severity labels and at least a short-term mitigation or workaround for critical/user-blocking issues.
- Validation notes, test artifacts (logs, telemetry summaries, crash dumps), and a short runbook for reproducing top-5 critical issues are added to the task file or linked artefacts.

## Context & Links 🔗
- Existing Phase artifacts: `.instructions/artefacts/` (review for prior test plans and spike outputs).
- Current test infra notes: `.instructions/` and any existing soak/validation scripts in `scripts/` and `tools/` folders.
- Distribution channels: community Discord / forum / mailing list (confirm list or channel and moderator/contacts before distribution).

## Plan / Approach 🛠️
1. Draft a compatibility matrix template (columns: Game, Engine, Renderer, OS, GPU, Observed Behavior, Notes, Test Build ID).
2. Select an initial set of 20 games across engines and genres; coordinate with maintainers for test permissions / reproductions if needed.
3. Prepare three representative test machines (low, mid, high) with instrumented beta builds and enable telemetry/logging (performance counters, memory snapshots, crash dumps).
4. Implement and run unattended 8-hour soak tests on each machine, capturing periodic memory snapshots and continuous performance traces.
5. Add automated checks for memory growth and a post-run script to summarize findings (max/min/mean memory, allocations over time, crash counts).
6. Execute sleep/wake scenarios: sleep immediately after overlay starts, sleep during gameplay, and wake while overlay is active; collect logs and user-visible symptoms.
7. Package and publish a beta build to the community channel with clear repro & feedback instructions and an issue template for bug reports.
8. Collect reports and telemetry for 7 days, triage issues, and create reproducible bug reports with reproduction steps and attachments.
9. Summarize findings, mark top-5 critical issues with mitigation steps, and recommend fixes or follow-up tasks (e.g., a memory leak investigation task).

## Validation Notes 🔍
- Soak tests
  - Use an automated runner (scripted or CI job) to launch the game and overlay, run a scripted play or idle scenario for 8 hours, and gracefully stop.
  - Capture periodic memory snapshots (e.g., every 30 minutes) and record working set / private bytes / GPU memory.
  - Flag runs where memory grows > 10% over baseline or exhibits sustained linear growth (investigate further).
- Memory leak checks
  - Use WinDbg / ProcDump to capture heap stacks on OOM or runaway growth.
  - Prefer instrumented builds that keep allocation stacks or use diagnostics APIs to resolve origins.
- Sleep/wake tests
  - Execute tests on Win10 and Win11; test both S0->S3 sleep and lid-close scenarios (for laptops).
  - Check hotkeys, overlay visibility, and input focus after wake.
- Community beta distribution
  - Provide clear opt-in instructions and an issue template: steps, OS/GPU, logs, and the test build ID.
  - Encourage attaching `perf.zip`/log bundles; provide a small script to collect logs automatically.
- Issue collection & triage
  - Create a GitHub issue label set (e.g., `beta`, `regression`, `crash`, `memory-leak`, `sleep-wake`) and a triage board.
  - Triage incoming issues daily and escalate critical issues immediately.

## Notes / Risks ❗
- Telemetry privacy: confirm what data may be collected and redact PII; obtain consent in the beta invite.
- Build stability: ensure builds include debug symbols and crash reporting hooks to maximize triage value.
- Resource/time: soak tests require machine time; consider cloud or automated lab scheduling if available.

## Next Steps ➡️
- Assign an owner and confirm distribution channel and invite list.
- Prepare a draft compatibility matrix and a beta invite message for review.
- Run an initial pilot soak test (4 hours) to validate the scripts, then schedule full 8-hour runs.

---

**Suggested Adjacent Tasks**
- Add automated telemetry dashboard / alerting for memory regressions.
- Create a follow-up task to investigate any confirmed memory leaks with a narrower scope (heap pin-pointing, repro minimization).
- Add unit/integration tests to exercise known problematic code paths discovered during beta.

**How to Validate (quick)**
1. Run the pilot soak (4 hours) and confirm logs and memory snapshots are collected.
2. Publish beta build and verify at least 30 unique sessions and 10 reported issues in 7 days.
3. Confirm compatibility matrix is at least 80% filled for the initial 20 games.

