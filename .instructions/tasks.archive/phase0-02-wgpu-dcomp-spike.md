---
schema: task/v1
id: task-000001
title: "Spike: WGPU + DirectComposition surface integration (phase0-02)"
type: research
status: archived
priority: medium
owner: "executive2"
skills: ["design", "debug"]
depends_on: []
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Context

This is a small engineering spike to determine whether we can host a wgpu (DX12) surface on a DirectComposition visual (SurfaceTarget::Visual) and render fully transparent content into it. This is an exploratory step in Phase0 of the GLASS overlay work: we want a minimal reproducible program that demonstrates the rendering path and determines a fallback strategy.

Plan artefact: `.instructions/artefacts/phase0-architecture.md` (if present) — add/update after this spike as needed.

## Goal ✅

Create a minimal Windows program that:
- Creates an HWND with the required window styles for DirectComposition hosting and transparent composition.
- Ensures the process is DPI-aware (PerMonitorAwareV2 / PMv2).
- Initializes DirectComposition (device, target, visual).
- Creates a wgpu DX12 instance and a wgpu Surface that targets the DirectComposition visual (SurfaceTarget::Visual).
- Clears the surface to fully transparent and commits/flags the composition so the window is visually transparent.

## Acceptance criteria ✅

1. A minimal program builds and runs on Windows (Win10/Win11) without crashes.
2. The program sets PerMonitorAwareV2 (via API or manifest) successfully.
3. The program creates a Window (HWND) with the styles commonly required for composition hosting (document styles to try: e.g., WS_POPUP, WS_EX_NOREDIRECTIONBITMAP, optionally WS_EX_LAYERED; note: exact list is implementation detail to verify).
4. DirectComposition initialization succeeds (DComposition device / target / visual created and attached to the HWND).
5. wgpu (DX12 backend) instance is created and a wgpu Surface is created bound to the DirectComposition visual (i.e., SurfaceTarget::Visual path succeeds).
6. The wgpu render pass clears the surface to transparent (alpha = 0) and the visual composition result displays transparency in the window.
7. A clear fallback path exists: if SurfaceTarget::Visual fails, the program attempts creating a wgpu Surface from the HWND (HWND surface) and uses DwmExtendFrameIntoClientArea to get a transparent client area; this fallback is exercised and validated.
8. If both the Visual-target surface and the HWND-based surface + DWM extension fail, the spike is stopped and an explicit failure report is produced (logs, reproduction steps, and suggested next actions).

## Plan / Approach 🔧

1. Create a minimal Win32 app skeleton that:
   - Sets PerMonitorAwareV2 (call SetProcessDpiAwarenessContext or use manifest).
   - Registers and creates an HWND with the styles to test (document choices and rationale in Notes).
   - Hooks a simple message loop.

2. Initialize DirectComposition:
   - Create DCompositionDevice.
   - Create a Target for the HWND and a Visual.
   - Attach the Visual to the Target.

3. Try primary approach (SurfaceTarget::Visual):
   - Create a wgpu Instance with the DX12 backend.
   - Create a wgpu Surface that targets the DirectComposition visual (platform-specific API or helper to use SurfaceTarget::Visual).
   - Acquire swapchain/surface and perform a clear to transparent, submit, and commit the DirectComposition device.
   - Validate visible transparency.

4. Fallback approach (if Visual fails):
   - Create a wgpu Surface from HWND directly (HWND surface path).
   - Use DwmExtendFrameIntoClientArea to extend frame and allow transparent client area.
   - Clear to transparent and validate.

5. If both approaches fail, stop and report with logs and suggested next steps.

## Validation / How to verify ✅

- Build & run the program; it should not crash.
- Observe the window: the area covered by the visual should render transparent where cleared.
- Add logging for every major step with success/failure and HRESULT or error text for DirectComposition and wgpu surface creation.
- When successful: attach a short screen-recording or screenshot showing the transparent overlay (or a programmatic pixel-sample test if possible).
- When failing: collect logs, HRESULTs, wgpu error messages, OS version, and GPU adapter info; include small repro steps.

## Decision gate ⚖️

- If the SurfaceTarget::Visual approach succeeds → document code flow and acceptance criteria met; propose next step to integrate into the overlay prototype.
- If SurfaceTarget::Visual fails, try the HWND surface + DWM extension fallback. If fallback succeeds → document fallback and determine whether it is acceptable for product constraints.
- If both fail → stop the spike and produce a formal report with logs and suggested next experiments (e.g., investigate IDCompositionSurface usage, alternate DXGI interop routes, or other composition APIs).

## Attempts / Log

- (empty) — to be filled during the spike run.

## Notes / Discoveries

- Candidate window styles to document and test: `WS_POPUP`, `WS_EX_NOREDIRECTIONBITMAP`, `WS_EX_LAYERED`. Testing will reveal the minimal set required.
- DPI: call `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` as earliest possible step, or use a manifest entry; note behavior differences across Windows 10/11.
- Surface alpha: ensure swapchain/surface format and present allow alpha channel and that DirectComposition compositing preserves alpha.

## Next Steps

- Assign an owner and run the spike locally.
- Record the results and update the plan artefact: `.instructions/artefacts/phase0-architecture.md` with outcome and recommended route.

---

**If you want, I can:**
1. Create this task file (done).
2. Also add a `test-task` to ensure a reproducible validation workflow for this spike (optional).

