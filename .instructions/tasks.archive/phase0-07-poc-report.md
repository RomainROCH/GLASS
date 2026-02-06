---
schema: task/v1
id: task-000434
title: "Compile PoC results: PoC report and go/no-go decision"
type: docs
status: archived
priority: medium
owner: "executive2"
skills: ["docs", "planning-feature"]
depends_on: []
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Context

We ran a Proof-of-Concept (PoC) for rendering overlays across a matrix of games and GPUs (see related PoC spike tasks):
- `.instructions/tasks/phase0-01-scaffolding.md` ✅
- `.instructions/tasks/phase0-02-wgpu-dcomp-spike.md` ✅
- `.instructions/tasks/phase0-03-triangle-render.md` ✅
- `.instructions/tasks/phase0-04-passthrough-window.md` ✅

This task compiles the PoC results into a single, actionable report that documents pass/fail results per game/GPU, key metrics, screenshots, logs, and a clear recommendation: proceed, iterate, or stop.

## Goal

Create a PoC report (Markdown or PDF) that documents results for each tested game/GPU combination and concludes with a go/no-go decision and recommended next steps.

## Acceptance Criteria

1. Report exists at `.artifacts/poc-report.md` (or `.artifacts/poc-report.pdf`) and is linked from this task.
2. Contains a pass/fail matrix covering every tested Game × GPU row and the overall pass rate.
3. For each Game × GPU entry, includes:
   - Test status: Pass | Fail | Flaky
   - Key metrics: average FPS, median frame time, frame time stdev, CPU% and GPU% peak/average, memory usage, and number of crashes/hangs
   - 2–3 representative screenshots (one showing expected overlay rendering, one showing an observed failure if applicable)
   - Links to raw logs and recordings (video or trace files)
4. A summary section with:
   - Overall recommendation: Proceed / Iterate / Stop
   - Rationale for the recommendation (risk, blocker list, needed effort)
   - Clear next steps and owners for each next step
5. Validation checklist included and all items are verifiable by reviewer within 10 minutes using provided artifacts.

## Plan / Approach

1. Collect artifacts from test runs (logs, screenshots, traces, recordings) into `.artifacts/poc/<timestamp>/`.
2. For each Game × GPU:
   - Extract metrics from traces/logs (use existing scripts or add simple parsing scripts).
   - Select representative screenshots and short recordings.
   - Determine status: Pass if stable with acceptable performance and no visual defects; Fail if crashes, major visual defects, or unacceptable performance; Flaky if inconsistent.
3. Produce a summary matrix (table) and visual charts (e.g., FPS distribution by GPU) to illustrate trends.
4. Draft the recommendation and risk section, including mitigation steps and estimated effort.
5. Review internally and attach a one-line sign-off (owner + date).

## Validation Notes / How to Verify ✅

- Open `.artifacts/poc-report.md` and confirm the pass/fail matrix includes all tested permutations.
- Randomly pick 3 entries marked as Pass and 3 marked as Fail/Flaky and verify artifacts exist and support the verdict (screenshots, logs, recordings).
- Run the provided metric-parsing script against one sample log and confirm metrics match the report numbers.
- Confirm the recommendation section lists specific blockers (if any), not just vague language.

## Metrics to Capture

- Average FPS, median frame time, frame time standard deviation
- CPU usage (% avg / % peak)
- GPU usage (% avg / % peak)
- Memory usage (RSS/VRAM peaks)
- Crash count / hang occurrences / recovery attempts
- Duration of the test run and sample size (seconds, frames)

## Deliverables

- `.artifacts/poc-report.md` (primary deliverable)
- `.artifacts/poc/<timestamp>/` folder with all raw artifacts (screenshots, logs, recordings)
- Short changelog or appendix describing how metrics were computed and any parsing scripts used
- One-line owner sign-off and final decision (Proceed / Iterate / Stop)

## Notes / Links

- Related spike tasks: see `.instructions/tasks/phase0-01-scaffolding.md`, `.instructions/tasks/phase0-02-wgpu-dcomp-spike.md`, `.instructions/tasks/phase0-03-triangle-render.md`, `.instructions/tasks/phase0-04-passthrough-window.md`.
- Suggested storage: add artifacts under `.artifacts/poc/` or a similar repo-tracked folder; if large video artifacts are produced, upload to shared drive and link from the report.

## Next Steps

- When the report is complete, create follow-up tasks for any required engineering work (bug fixes, performance tuning, additional PoC rounds) with clear owners and estimates.

---

**How to validate this task (for the reviewer):** Open this file -> confirm the report file exists at the path above and matches the Acceptance Criteria -> check 3 random artifacts.
