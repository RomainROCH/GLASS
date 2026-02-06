---
schema: task/v1
id: task-000434
title: "HDR + DPI diagnostics: Detect HDR via IDXGIOutput6 & scRGB surfaces, explicit SDR fallback, and robust diagnostics on fatal GPU errors"
type: feature
status: not-started
priority: medium
owner: "unassigned"
skills: ["debug", "logging-observability", "design"]
depends_on: []
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Goal
Add reliable HDR detection and DPI/resolution independence to the overlay renderer and provide a deterministic, actionable diagnostics dump on fatal GPU errors. Specifically:
- Detect HDR-capable outputs using `IDXGIOutput6` (or higher) and prefer scRGB-capable surfaces where available.
- Provide an explicit, auditable SDR fallback path when HDR/scRGB isn't available or when running under environments that require SDR.
- Ensure DPI awareness and resolution-independent UI scaling (logical units) so overlays look correct at arbitrary DPI and output resolutions.
- On fatal GPU/display errors, generate a diagnostic dump containing GPU/adapter details, Windows version/build, DWM/composition state, active display color space and HDR state, DPI and scaling info, and any relevant DXGI info.

## Acceptance Criteria ✅
1. HDR / scRGB detection
   - The app queries outputs using `IDXGIOutput6` (or newer) where available and determines whether the output supports scRGB / HDR-type color spaces.
   - When an HDR-capable output is found, the renderer is able to create and use an scRGB-capable surface or an appropriate HDR DXGI color space.
   - If scRGB/HDR surface creation fails, the app logs a clear reason and falls back to the explicit SDR path.

2. Explicit, auditable SDR fallback
   - There is a deterministic SDR code path (no implicit silent tone-mapping) that is used when HDR is unavailable or explicitly disabled.
   - The active color pipeline (HDR or SDR) is recorded in logs and the fatal diagnostics dump.
   - A command-line flag or runtime toggle exists to force HDR on/off for testing and validation.

3. DPI awareness & resolution independence
   - Overlay layout is based on logical units (DIPs / scaled coordinates) rather than raw pixels, and scales correctly under common Windows DPI scaling factors (100%, 150%, 200%, etc.).
   - On high-DPI displays the overlay appears visually consistent (fonts, UI elements, hit areas unaffected by DPI) and no pixelation issues are observed at common scale factors.
   - Process DPI awareness is set appropriately (per-monitor v2 if feasible) and recorded in logs.

4. Diagnostics dump on fatal GPU/display errors
   - On a fatal GPU/display error (device lost, DXGI errors, or compositor failures), the app writes a diagnostics dump (JSON + optional human-readable text) that includes at minimum:
     - Timestamp
     - GPU adapter: Vendor ID, Device ID, adapter description, driver version
     - DXGI device/device removal reason (if available)
     - Windows version and build (e.g., major/minor/build/UBR)
     - DWM composition state (enabled/disabled) and basic DWM info
     - Attached outputs queried via DXGI/IDXGIOutput*: for each output: name, monitor handle, resolution, refresh, color space / colorimetry, HDR-capable flag, current brightness (if available)
     - Current renderer color pipeline: HDR vs SDR, DXGI color space enum, whether scRGB surface is in use
     - Process DPI awareness and effective DPI for the output(s)
     - Helpful pointers / next steps (e.g., "Try disabling HDR in Windows Display Settings to force SDR and re-run", or driver upgrade suggestion)
   - The dump is persisted to a deterministic location and logged to the standard error / telemetry channel for fast triage.

5. Tests & validation hooks
   - Manual validation steps are documented (see Validation Notes). A test toggle is available to force HDR on/off and to emit the diagnostics dump on demand.

## Context & Links 🔗
- Windows DXGI docs: `IDXGIOutput6` and `IDXGIOutput::GetDesc` / `IDXGIOutput6::GetDesc1` — use Microsoft docs for up-to-date signatures.
- Color spaces & HDR references: Microsoft docs on HDR, scRGB, and DXGI color space enums (e.g., DXGI_COLOR_SPACE_* constants).
- DWM composition APIs: `DwmIsCompositionEnabled`, `DwmGetColorizationColor`, `DwmGetCompositionTimingInfo`.
- Existing project tasks (Phase 0 rendering and scaffolding) that should be completed or referenced before implementation: `phase0-03-triangle-render.md`, `phase0-01-scaffolding.md`.

## Plan / Approach 🛠️
1. Investigation & design (small spike)
   - Prototype a minimal probe that enumerates adapters and calls `IDXGIOutput6` methods to detect color space / scRGB support and logs results.
   - Decide on the exact set of properties to include in diagnostics dump (schema below).
2. Implementation
   - Add an `hdr_detection` module responsible for:
     - Enumerating DXGI adapters and outputs, preferring `IDXGIOutput6` when available.
     - Exposing a deterministic API: `enum DisplayCapability { HDR, SDR, UNKNOWN }` and a `PreferredSurfaceFormat` result (e.g., scRGB preferred / sRGB fallback).
     - Creating surfaces that request scRGB where possible and falling back to SDR surfaces with explicit color space selection.
   - Add a global `diagnostics` utility that can assemble and persist the error dump (JSON schema defined in Plan). Include necessary helpers to stringify DXGI enums.
   - Add a `--force-hdr`, `--force-sdr`, and `--emit-diagnostics` runtime flags for testing.
   - Ensure process DPI awareness is configured early (application manifest and runtime checks), and add functions to read the effective DPI for each monitor.
3. Logging & observability
   - Log HDR detection results and chosen pipeline at startup and on display change events.
   - On fatal errors, call diagnostics utility synchronously to capture a dump before exit.
4. Validation hooks & manual tests
   - Expose an in-app or CLI command to force-emission of the diagnostics dump for support/debug scenarios.
   - Add a short dev-mode test that runs the detection logic and validates expected behavior in known environments (e.g., returns HDR on test machine with HDR enabled).

## Diagnostic JSON Schema (suggested)
{
  "timestamp": "2026-02-06T...Z",
  "os": { "productName": "Windows 10/11", "build": 22000, "ubr": 0 },
  "gpu": { "vendorId": 0x10DE, "deviceId": 0x1E07, "description": "NVIDIA ...", "driverVersion": "536.67" },
  "dxgi": { "deviceRemovedReason": null, "lastError": "DXGI_ERROR_DEVICE_REMOVED" },
  "dwm": { "compositionEnabled": true, "colorizationColor": "#..." },
  "outputs": [ { "monitor": "\\.\DISPLAY1", "resolution": "3840x2160", "refreshHz": 60, "hdrCapable": true, "colorSpace": "DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020", "scRGBSupported": true } ],
  "process": { "dpiAwareness": "per-monitor-v2", "effectiveDpi": 150 },
  "activeColorPipeline": "HDR/scRGB"
}

## Validation Notes 🔍
- Manual checks:
  1. On an HDR-capable system with HDR enabled in Windows, start the app and verify logs show an HDR-capable output and that the HDR/scRGB pipeline was selected.
  2. Force SDR (`--force-sdr`) and confirm SDR pipeline is used and recorded in logs/dump.
  3. Toggle Windows HDR setting while running and verify the app detects display changes and logs new pipeline decision.
  4. Run on common DPI scale factors (100%, 150%, 200%) and verify overlay layout & text look consistent and hit areas match cursor locations.
  5. Simulate a device removal / DXGI device lost condition (where feasible) and verify the diagnostics dump is emitted with the required fields.

- Automated/CI:
  - Add a small unit/integration test that exercises the detection module against mocked DXGI objects where possible.
  - For real-device validation, include a manual test checklist that can be executed on a Windows machine with HDR-capable display.

## Notes / Questions ❓
- Which exact output properties do we want to capture beyond the suggested list (e.g., HDR metadata blocks, EDR/brightness, color primaries)?
- Is `per-monitor v2` DPI awareness acceptable or do we prefer a different default? (per-monitor v2 recommended for overlays)
- Telemetry: do we want to upload diagnostics dumps to a centralized service or keep them local only? (privacy / PII considerations)

## Next Steps ➡️
1. Triage this task, confirm owner, and run the initial probe to validate `IDXGIOutput6` usage on a local HDR-capable machine.
2. Implement detection + SDR fallback and add the diagnostics persister.
3. Add validation hooks and the testing checklist; iterate on observed edge cases.

---

## Attempts / Log
- Created: 2026-02-06

## Contacts
- If you'd like me to implement this, tell me who to assign and whether to include telemetry upload for diagnostic dumps (yes/no).
