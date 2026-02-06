---
schema: task/v1
id: task-000001
title: "Implement passthrough window behavior: WM_NCHITTEST => HTTRANSPARENT"
type: feature
status: archived
priority: medium
owner: "executive2"
skills: ["feature-creator", "system-editor"]
depends_on: []
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Context

We want the overlay window to be visually topmost while not interfering with input or focus. The core behavior is that when the overlay is visible it should be "click-through" (mouse and pointer events should pass to the windows underneath), should not take focus or appear in Alt+Tab, and should behave sensibly when the OS or another application is using exclusive fullscreen.

Relevant platform docs:
- WM_NCHITTEST (returns HTTRANSPARENT to make the window click-through): https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-nchittest ✅
- HTTRANSPARENT explanation: https://learn.microsoft.com/en-us/windows/win32/winmsg/httransparent
- Window styles: WS_EX_TRANSPARENT, WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_EX_TOOLWINDOW: https://learn.microsoft.com/en-us/windows/win32/winprog/window-styles
- SetWindowLongPtr / SetWindowPos: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowlongptrw
- Exclusive fullscreen / DirectX notes (limitations for overlays): https://learn.microsoft.com/en-us/windows/win32/direct3darticles/using-direct3d

Notes:
- If appropriate, prefer returning HTTRANSPARENT from WM_NCHITTEST *for overlay client area hits* rather than a global WS_EX_TRANSPARENT change so we preserve expected layering and mouse cursor behavior.
- Consider WS_EX_NOACTIVATE to avoid the overlay ever becoming the foreground window.

## Goal

Implement and document a robust passthrough (click-through & input-transparent) behavior for the overlay window so that:
- Mouse/pen/touch events pass through overlay regions (WM_NCHITTEST => HTTRANSPARENT where applicable).
- The overlay never steals focus or becomes the foreground window when interacted with.
- Alt+Tab behavior is unaffected and the overlay does not appear in Alt+Tab.
- Topmost behavior is preserved for normal desktop windows while defining and documenting acceptable behavior when an exclusive fullscreen app is running.

## Acceptance criteria ✅

1. Click-through behavior:
   - When the overlay window receives WM_NCHITTEST for a client-area hit, the handler returns HTTRANSPARENT and the input is delivered to the underlying window.
   - Manual verification: clicking through the overlay interacts with the underlying app (e.g., Notepad, browser) as expected.

2. No focus steal:
   - Clicking or interacting with the overlay does not change the active (foreground) window.
   - `GetForegroundWindow()` still points at the previously active window after interacting through the overlay.

3. Alt+Tab unaffected:
   - Pressing Alt+Tab cycles windows normally — the overlay is not presented as a switchable application window (if needed, set WS_EX_TOOLWINDOW or other appropriate style).

4. Topmost behavior validated:
   - The overlay remains visually on top of standard windows (SetWindowPos with TOPMOST) but does not capture input.
   - When another window is explicitly made topmost (or a fullscreen window), behavior is consistent and documented.

5. Exclusive fullscreen behavior documented and validated:
   - Document expected limitations when another process uses exclusive fullscreen (e.g., game that uses DirectX exclusive mode). Specify whether overlay will:
     - remain on top but may not be visible or composited (documented limitation), or
     - automatically hide/disable while exclusive fullscreen is active (preferred if implemented), or
     - implement a fallback (borderless fullscreen behavior) if applicable.
   - Manual test: run an app in exclusive fullscreen and verify the chosen behavior.

## Validation / Testing notes (manual steps)

1. Click-through tests
   - Launch a target app (Notepad). Move the overlay over a button or editable area and click — verify the underlying app receives the click and state changes.
   - Use mouse, touch, and pen if supported.

2. Focus tests
   - With another app focused, click over the overlay. Assert the previously focused app remains focused (no foreground change).
   - Use tooling (e.g., a small test app that logs GetForegroundWindow or GetActiveWindow) to confirm.

3. Alt+Tab tests
   - Bring several apps into the foreground. Press Alt+Tab and ensure overlay does not appear and switching behaves normally.

4. Topmost tests
   - Open other topmost windows (e.g., Task Manager set to "Always on top") and verify overlay ordering behavior per design.

5. Exclusive fullscreen tests
   - Launch an application in exclusive fullscreen (e.g., a DirectX sample or game). Confirm chosen behavior (hide overlay, remain but not composited, or documented limitation). Document observed outcomes and any platform-imposed limitations.

6. Automation ideas
   - Add a small unit/integration test harness (if feasible) that can run on CI or a developer machine to validate WM_NCHITTEST returns for sample hit coordinates and that window styles include WS_EX_NOACTIVATE when expected.

## Implementation hints / notes

- Primary implementation surface: overlay window message handler (WM_NCHITTEST) — returning HTTRANSPARENT for client-area hits.
- Consider WS_EX_NOACTIVATE and WS_EX_TOOLWINDOW so the overlay does not appear in Alt+Tab and cannot be activated.
- For layered windows, the Windows compositor and exclusive fullscreen may limit visibility or ordering — document these as acceptance test notes.
- Ensure these changes are feature-gated behind a config flag or platform check (Windows only), and add telemetry/logging to record observed behavior in the field.

## Links / References
- WM_NCHITTEST: https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-nchittest
- HTTRANSPARENT: https://learn.microsoft.com/en-us/windows/win32/winmsg/httransparent
- Extended Window Styles: https://learn.microsoft.com/en-us/windows/win32/winprog/window-styles
- SetWindowLongPtr / SetWindowPos: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowlongptrw
- Notes on exclusive fullscreen and overlays: https://learn.microsoft.com/en-us/windows/win32/direct3darticles/using-direct3d

## Next steps
1. Implement WM_NCHITTEST => HTTRANSPARENT behavior for overlay client area and add WS_EX_NOACTIVATE where appropriate.
2. Run the manual validation steps above, document test results in this task file under "Attempts / Log".
3. If exclusive fullscreen requires hiding the overlay, add an explicit detection + disable path and update acceptance criteria.

## Attempts / Log

- None yet.

## Notes / Discoveries

- Document platform-imposed limitations observed during testing and any workarounds.

## Next Steps

- Assign owner, implement change, and run validation tests.
