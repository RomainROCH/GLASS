//! Overlay window creation and message pump.
//!
//! Creates a topmost, click-through, non-activatable HWND.
//! Uses `WS_EX_LAYERED | WS_EX_TRANSPARENT` for input pass-through and
//! `WS_EX_NOREDIRECTIONBITMAP` to suppress the GDI surface.
//! All visual content comes from DirectComposition (see `compositor.rs`).
//! System tray icon for clean exit (right-click → Quit).

use crate::renderer::Renderer;
use crate::test_mode;
use std::mem;
use tracing::{info, warn};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// System tray constants
const WM_TRAYICON: u32 = 0x8000 + 1; // WM_APP + 1
const IDM_EXIT: usize = 1001;
const TRAY_ICON_ID: u32 = 1;

/// Set PerMonitorAwareV2 DPI awareness. Must be called before any window creation.
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

/// Creates the overlay HWND with required styles.
pub fn create_overlay_window() -> Result<HWND, glass_core::GlassError> {
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

        // WS_EX_LAYERED + WS_EX_TRANSPARENT: the combination makes the
        // Window Manager skip this HWND for all pointer hit-testing, providing
        // full click-through regardless of alpha value.
        // WS_EX_NOREDIRECTIONBITMAP: no GDI surface — all visual content
        // comes from the DirectComposition visual (see compositor.rs).
        let ex_style = WS_EX_TOPMOST
            | WS_EX_TOOLWINDOW
            | WS_EX_NOACTIVATE
            | WS_EX_TRANSPARENT
            | WS_EX_LAYERED
            | WS_EX_NOREDIRECTIONBITMAP;

        // Create without WS_VISIBLE — we show after SetLayeredWindowAttributes
        // to avoid a flash of opaque GDI surface.
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

        // Activate the layered window.  alpha=255 keeps the DComp visual
        // fully visible; input pass-through comes from WS_EX_TRANSPARENT,
        // not from the alpha byte.  NOREDIRECTIONBITMAP means there is no
        // GDI surface for this alpha to affect.
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);

        // Set window title (with [MODE TEST] prefix in test_mode builds)
        {
            let title = format!("{}GLASS Overlay\0", test_mode::TITLE_PREFIX);
            let title_wide: Vec<u16> = title.encode_utf16().collect();
            let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(title_wide.as_ptr()));
        }

        // Now show the window (non-activating) so it doesn't steal focus.
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        info!(
            "Overlay window created: {}x{}, HWND={:?}",
            screen_w, screen_h, hwnd
        );

        // Add system tray icon for clean exit
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

        // Tooltip: test-mode aware title
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
        // Required: SetForegroundWindow before TrackPopupMenu, otherwise menu won't dismiss.
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_BOTTOMALIGN, pt.x, pt.y, Some(0), hwnd, None);
        let _ = DestroyMenu(menu);
    }
}

/// Window procedure — click-through + tray icon + retained rendering.
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // Full click-through for overlay
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),

        // System tray icon callback
        x if x == WM_TRAYICON => {
            let mouse_msg = lparam.0 as u32;
            if mouse_msg == WM_RBUTTONUP {
                show_tray_menu(hwnd);
            }
            LRESULT(0)
        }

        // Menu command (from tray context menu)
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as usize;
            if id == IDM_EXIT {
                info!("Quit requested via tray icon");
                remove_tray_icon(hwnd);
                unsafe { let _ = DestroyWindow(hwnd); }
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            remove_tray_icon(hwnd);
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }

        // Validate paint region so Windows stops sending WM_PAINT
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe { let _ = BeginPaint(hwnd, &mut ps); }
            unsafe { let _ = EndPaint(hwnd, &ps); }
            LRESULT(0)
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Blocking message loop. Retained: only re-renders on WM_PAINT.
pub fn run_message_loop(renderer: &mut Renderer) {
    info!("Entering message loop (retained rendering)");
    let mut msg = MSG::default();
    let mut frame_count: u64 = 0;

    unsafe {
        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if ret == FALSE {
                break; // WM_QUIT
            }

            // Re-render on size change
            if msg.message == WM_SIZE || msg.message == WM_DISPLAYCHANGE {
                if let Err(e) = renderer.resize() {
                    warn!("Resize failed: {e}");
                }
                if let Err(e) = renderer.render() {
                    warn!("Render after resize failed: {e}");
                }
                frame_count += 1;
            }

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);

            // Track allocation counts per frame (debug only)
            #[cfg(all(debug_assertions, feature = "alloc-tracking"))]
            {
                let allocs = crate::alloc_tracker::frame_alloc_count();
                if frame_count > 2 && allocs > 0 {
                    warn!("Steady-state allocation detected: {allocs} allocations in frame {frame_count}");
                }
                crate::alloc_tracker::reset_frame_count();
                frame_count += 1;
            }
        }
    }

    info!("Message loop exited after {frame_count} frames");
}
