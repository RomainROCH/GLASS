//! Overlay window creation and message pump.
//!
//! Creates a topmost, click-through, non-activatable HWND using
//! `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOREDIRECTIONBITMAP`.
//! All visual content comes from DirectComposition (see [`Compositor`]).
//!
//! Supports two input modes:
//! - **Passive (Mode A)** — default; fully click-through.
//! - **Interactive (Mode B)** — hotkey-triggered; accepts mouse on interactive
//!   rects, auto-reverts after timeout.
//!
//! System tray icon provides a clean exit (right-click → Quit).

use crate::input::{
    InputMode, OverlayInputState, HOTKEY_ID, INTERACTIVE_TIMER_ID,
    WM_GLASS_MODE_INTERACTIVE, WM_GLASS_MODE_PASSIVE,
};
use crate::layout::LayoutManager;
use crate::renderer::{Renderer, MODULE_UPDATE_TIMER_ID};
use crate::test_mode;
use std::mem;
use std::time::Instant;
use tracing::{debug, error, info, warn};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// System tray constants
const WM_TRAYICON: u32 = 0x8000 + 1; // WM_APP + 1
const IDM_EXIT: usize = 1001;
const TRAY_ICON_ID: u32 = 1;

/// Set PerMonitorAwareV2 DPI awareness.
///
/// **Must** be called before any window creation. Calling after window
/// creation is undefined behaviour on Windows.
pub fn set_dpi_awareness() {
    unsafe {
        let result = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        if result.is_err() {
            warn!("SetProcessDpiAwarenessContext failed (may already be set): {:?}", result);
        } else {
            info!("DPI awareness set to PerMonitorAwareV2");
        }
    }
}

/// Show a modal error dialog to the user.
///
/// Uses `MessageBoxW` with `MB_OK | MB_ICONERROR`. Returns when the user
/// dismisses the dialog.
pub fn show_error_dialog(title: &str, message: &str) {
    unsafe {
        let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let msg_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = MessageBoxW(
            None,
            windows::core::PCWSTR(msg_wide.as_ptr()),
            windows::core::PCWSTR(title_wide.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// Creates the overlay HWND with all required extended window styles.
///
/// The window is fullscreen, click-through, topmost, tool-window (no alt-tab),
/// and uses `WS_EX_NOREDIRECTIONBITMAP` so the GDI surface is suppressed.
/// A system tray icon is added for clean exit.
/// A global hotkey is registered for interactive mode (unless test_mode).
///
/// # Arguments
/// * `timeout_ms` — interactive mode timeout in milliseconds.
/// * `hotkey_vk` — virtual key code for the toggle hotkey (e.g. `0x7B` = F12).
/// * `hotkey_modifiers` — Win32 `MOD_*` modifier flags (0 = no modifier).
pub fn create_overlay_window(
    timeout_ms: u32,
    hotkey_vk: u32,
    hotkey_modifiers: u32,
) -> Result<HWND, glass_core::GlassError> {
    unsafe {
        let hinstance = GetModuleHandleW(None)
            .map_err(|e| glass_core::GlassError::WindowCreation(format!("GetModuleHandle: {e}")))?;

        let class_name = windows::core::w!("GLASS_OVERLAY");

        let wc = WNDCLASSEXW {
            cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW).ok().unwrap_or_default(),
            ..Default::default()
        };

        let atom = RegisterClassExW(&wc);
        if atom == 0 {
            return Err(glass_core::GlassError::WindowCreation(
                "RegisterClassExW returned 0".into(),
            ));
        }

        // Get primary monitor dimensions for fullscreen overlay
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);

        let ex_style = WS_EX_TOPMOST
            | WS_EX_TOOLWINDOW
            | WS_EX_NOACTIVATE
            | WS_EX_TRANSPARENT
            | WS_EX_LAYERED
            | WS_EX_NOREDIRECTIONBITMAP;

        // Create without WS_VISIBLE to avoid flash of opaque GDI surface.
        let hwnd = CreateWindowExW(
            ex_style,
            class_name,
            windows::core::w!("GLASS Overlay"),
            WS_POPUP,
            0,
            0,
            screen_w,
            screen_h,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
        .map_err(|e| glass_core::GlassError::WindowCreation(format!("CreateWindowExW: {e}")))?;

        // Activate the layered window — alpha=255 keeps DComp visual fully
        // visible; pass-through comes from WS_EX_TRANSPARENT.
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);

        // Set window title (with [MODE TEST] prefix in test_mode builds)
        {
            let title = format!("{}GLASS Overlay\0", test_mode::TITLE_PREFIX);
            let title_wide: Vec<u16> = title.encode_utf16().collect();
            let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(title_wide.as_ptr()));
        }

        // Store input state in GWLP_USERDATA for wnd_proc access
        let input_state = Box::new(OverlayInputState::new(timeout_ms));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(input_state) as isize);

        // Register the interactive-mode toggle hotkey (unless test_mode forces passthrough)
        if !test_mode::FORCE_INPUT_PASSTHROUGH {
            let mods = HOT_KEY_MODIFIERS(hotkey_modifiers);
            let result = RegisterHotKey(Some(hwnd), HOTKEY_ID, mods, hotkey_vk);
            if result.is_err() {
                warn!(
                    "Failed to register hotkey (vk=0x{:02X}, mods=0x{:X}): hotkey may be in use by another app",
                    hotkey_vk, hotkey_modifiers
                );
                // Mark interactivity as unavailable on this HWND
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayInputState;
                if !ptr.is_null() {
                    (*ptr).interactivity_available = false;
                }
            } else {
                info!(
                    "Global hotkey registered: vk=0x{:02X}, mods=0x{:X}",
                    hotkey_vk, hotkey_modifiers
                );
            }
        } else {
            info!("Test mode: hotkey registration skipped (FORCE_INPUT_PASSTHROUGH)");
        }

        // Show the window non-activating so it doesn't steal focus.
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        info!(
            "Overlay window created: {}x{}, HWND={:?}",
            screen_w, screen_h, hwnd
        );

        // System tray icon for clean exit
        add_tray_icon(hwnd);

        Ok(hwnd)
    }
}

/// Add a system tray icon with callback to our overlay window.
fn add_tray_icon(hwnd: HWND) {
    unsafe {
        let icon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();

        let mut nid = NOTIFYICONDATAW {
            cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAYICON,
            hIcon: icon,
            ..Default::default()
        };

        let tip = test_mode::TRAY_TOOLTIP;
        for (i, ch) in tip.encode_utf16().enumerate() {
            if i >= nid.szTip.len() - 1 {
                break;
            }
            nid.szTip[i] = ch;
        }

        if Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
            info!("System tray icon added");
        } else {
            warn!("Failed to add system tray icon");
        }
    }
}

/// Remove the system tray icon (called on exit).
fn remove_tray_icon(hwnd: HWND) {
    unsafe {
        let nid = NOTIFYICONDATAW {
            cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

/// Show context menu at cursor position for tray icon right-click.
fn show_tray_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        let _ = AppendMenuW(menu, MF_STRING, IDM_EXIT, windows::core::w!("Quit GLASS"));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_BOTTOMALIGN, pt.x, pt.y, Some(0), hwnd, None);
        let _ = DestroyMenu(menu);
    }
}

/// Retrieve the input state from GWLP_USERDATA.
///
/// Returns `None` during window creation (before state is set) or if the
/// pointer is null.
unsafe fn get_input_state(hwnd: HWND) -> Option<&'static mut OverlayInputState> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayInputState;
        if ptr.is_null() {
            None
        } else {
            Some(&mut *ptr)
        }
    }
}

/// Enter interactive mode: remove WS_EX_TRANSPARENT, start timeout timer.
unsafe fn activate_interactive_mode(hwnd: HWND, state: &mut OverlayInputState) {
    unsafe {
        // Remove WS_EX_TRANSPARENT so the window receives hit tests
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            ex_style & !(WS_EX_TRANSPARENT.0 as isize),
        );

        // Start (or reset) the timeout timer
        let _ = SetTimer(Some(hwnd), INTERACTIVE_TIMER_ID, state.timeout.as_millis() as u32, None);

        state.enter_interactive();

        // Post a custom message so the message loop can update the scene
        let _ = PostMessageW(Some(hwnd), WM_GLASS_MODE_INTERACTIVE, WPARAM(0), LPARAM(0));
    }
}

/// Enter passive mode: add WS_EX_TRANSPARENT, kill timeout timer.
unsafe fn activate_passive_mode(hwnd: HWND, state: &mut OverlayInputState) {
    unsafe {
        // Add WS_EX_TRANSPARENT back so the window is fully click-through
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            ex_style | WS_EX_TRANSPARENT.0 as isize,
        );

        // Kill the timeout timer
        let _ = KillTimer(Some(hwnd), INTERACTIVE_TIMER_ID);

        state.enter_passive();

        // Post a custom message so the message loop can update the scene
        let _ = PostMessageW(Some(hwnd), WM_GLASS_MODE_PASSIVE, WPARAM(0), LPARAM(0));
    }
}

/// Window procedure — mode-aware hit-testing, hotkey, tray icon.
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
    match msg {
        WM_NCHITTEST => {
            // During creation, before state is set, always pass through
            let Some(state) = get_input_state(hwnd) else {
                return LRESULT(HTTRANSPARENT as isize);
            };

            // In test mode or passive mode, always pass through
            if test_mode::FORCE_INPUT_PASSTHROUGH || state.mode == InputMode::Passive {
                return LRESULT(HTTRANSPARENT as isize);
            }

            // Interactive mode: check if cursor is over an interactive rect
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;

            if state.hit_tester.hit_test(x, y).is_some() {
                debug!("Hit test: HTCLIENT at ({x}, {y})");
                LRESULT(HTCLIENT as isize)
            } else {
                LRESULT(HTTRANSPARENT as isize)
            }
        }

        WM_HOTKEY => {
            if wparam.0 as i32 == HOTKEY_ID {
                if test_mode::FORCE_INPUT_PASSTHROUGH {
                    debug!("Hotkey pressed but FORCE_INPUT_PASSTHROUGH is active");
                    return LRESULT(0);
                }

                if let Some(state) = get_input_state(hwnd) {
                    if state.is_interactive() {
                        // Already interactive: reset the timer
                        let _ = SetTimer(
                            Some(hwnd),
                            INTERACTIVE_TIMER_ID,
                            state.timeout.as_millis() as u32,
                            None,
                        );
                        state.enter_interactive(); // resets timestamp
                        debug!("Interactive mode timer reset via hotkey");
                    } else {
                        activate_interactive_mode(hwnd, state);
                    }
                }
            }
            LRESULT(0)
        }

        WM_TIMER => {
            if wparam.0 == INTERACTIVE_TIMER_ID {
                if let Some(state) = get_input_state(hwnd) {
                    activate_passive_mode(hwnd, state);
                }
            }
            LRESULT(0)
        }

        x if x == WM_TRAYICON => {
            let mouse_msg = lparam.0 as u32;
            if mouse_msg == WM_RBUTTONUP {
                show_tray_menu(hwnd);
            }
            LRESULT(0)
        }

        WM_COMMAND => {
            let id = wparam.0 & 0xFFFF;
            if id == IDM_EXIT {
                info!("Quit requested via tray icon");
                remove_tray_icon(hwnd);
                // Unregister hotkey before destruction
                let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
                // Free input state
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayInputState;
                if !ptr.is_null() {
                    let _ = Box::from_raw(ptr);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            remove_tray_icon(hwnd);
            let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
            // Free input state if not already freed
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayInputState;
            if !ptr.is_null() {
                let _ = Box::from_raw(ptr);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }

        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut ps);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    } // unsafe
}

/// Blocking message loop. Retained rendering: only re-renders on
/// `WM_SIZE` / `WM_DISPLAYCHANGE` / mode transitions / module updates.
///
/// The `input_manager` updates the scene with a visual indicator when
/// the overlay enters interactive mode.
///
/// `module_registry` is ticked every ~100ms via a Win32 timer.
pub fn run_message_loop(
    renderer: &mut Renderer,
    input_manager: &mut crate::input::InputManager,
    layout_manager: &mut LayoutManager,
) {
    info!("Entering message loop (retained rendering + input modes + layout)");
    let mut msg = MSG::default();
    let mut frame_count: u64 = 0;
    let mut last_module_tick = Instant::now();

    unsafe {
        // Set a periodic timer for module updates (~100ms)
        let _ = SetTimer(None, MODULE_UPDATE_TIMER_ID, 100, None);

        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            // P0 fix: GetMessageW returns -1 on error — break to avoid infinite loop
            if ret == FALSE || ret.0 == -1 {
                if ret.0 == -1 {
                    error!("GetMessageW returned -1 (error) — exiting message loop");
                }
                break; // WM_QUIT or error
            }

            match msg.message {
                WM_SIZE | WM_DISPLAYCHANGE => {
                    // Extract new dimensions from WM_SIZE lparam
                    let (width, height) = if msg.message == WM_SIZE {
                        (
                            (msg.lParam.0 & 0xFFFF) as u32,
                            ((msg.lParam.0 >> 16) & 0xFFFF) as u32,
                        )
                    } else {
                        // For WM_DISPLAYCHANGE, query screen metrics
                        (
                            GetSystemMetrics(SM_CXSCREEN) as u32,
                            GetSystemMetrics(SM_CYSCREEN) as u32,
                        )
                    };

                    if let Err(e) = renderer.resize(width, height) {
                        warn!("Resize failed: {e}");
                    }
                    // Recalculate anchor-based layout for new dimensions
                    layout_manager.recalculate(
                        width as f32, height as f32, renderer.scene_mut(),
                    );
                    if let Err(e) = renderer.render() {
                        warn!("Render after resize failed: {e}");
                    }
                    frame_count += 1;
                }

                x if x == WM_GLASS_MODE_INTERACTIVE => {
                    info!("Mode transition → Interactive");
                    // Get screen dimensions for indicator sizing
                    let screen_w = GetSystemMetrics(SM_CXSCREEN) as f32;
                    let screen_h = GetSystemMetrics(SM_CYSCREEN) as f32;
                    input_manager.show_indicator(renderer.scene_mut(), screen_w, screen_h);

                    if let Err(e) = renderer.render() {
                        warn!("Render after mode change failed: {e}");
                    }
                    frame_count += 1;
                }

                x if x == WM_GLASS_MODE_PASSIVE => {
                    info!("Mode transition → Passive");
                    input_manager.hide_indicator(renderer.scene_mut());

                    if let Err(e) = renderer.render() {
                        warn!("Render after mode change failed: {e}");
                    }
                    frame_count += 1;
                }

                WM_TIMER if msg.wParam.0 == MODULE_UPDATE_TIMER_ID => {
                    let dt = last_module_tick.elapsed();
                    last_module_tick = Instant::now();
                    let dirty = layout_manager.update_all(renderer.scene_mut(), dt);
                    if dirty {
                        if let Err(e) = renderer.render() {
                            warn!("Render after module update failed: {e}");
                        }
                        frame_count += 1;
                    }
                    continue; // skip dispatch for our internal timer
                }

                _ => {}
            }

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Kill the module update timer
        let _ = KillTimer(None, MODULE_UPDATE_TIMER_ID);
    }

    info!("Message loop exited after {frame_count} frames");
}

/// Get a raw pointer to the [`OverlayInputState`] stored on a HWND.
///
/// Returns `None` if the pointer is null (e.g. during window creation).
///
/// # Safety
/// - Must only be called on the window-proc thread while the HWND is valid.
/// - Caller is responsible for ensuring no aliased mutable references exist
///   when dereferencing the returned pointer.
pub unsafe fn get_hwnd_input_state(hwnd: HWND) -> Option<*mut OverlayInputState> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayInputState;
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
}
