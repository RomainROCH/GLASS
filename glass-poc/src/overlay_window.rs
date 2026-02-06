//! Overlay window creation and message pump.
//!
//! Creates a transparent, topmost, click-through, non-activatable HWND.
//! Transparency via DWM composition (DwmExtendFrameIntoClientArea margins=-1).
//! System tray icon for clean exit (right-click → Quit).

use crate::renderer::Renderer;
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

// DWM FFI — direct binding avoids feature flag issues with `windows` crate
#[repr(C)]
struct DwmMargins {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

unsafe extern "system" {
    #[link_name = "DwmExtendFrameIntoClientArea"]
    fn DwmExtendFrameIntoClientArea(hwnd: HWND, margins: *const DwmMargins) -> windows::core::HRESULT;
}

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

        // No WS_EX_LAYERED — DWM composition handles transparency via alpha channel.
        // WS_EX_TRANSPARENT provides click-through (+ HTTRANSPARENT in wnd_proc).
        let ex_style = WS_EX_TOPMOST
            | WS_EX_TOOLWINDOW
            | WS_EX_NOACTIVATE
            | WS_EX_TRANSPARENT;

        let hwnd = CreateWindowExW(
            ex_style,
            class_name,
            windows::core::w!("GLASS Overlay"),
            WS_POPUP | WS_VISIBLE,
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

        // Extend DWM frame into client area for transparent composition.
        // With margins=-1, the entire client area is DWM "glass".
        // DWM reads the alpha channel from our framebuffer:
        //   alpha=0 → transparent, alpha>0 → visible.
        let margins = DwmMargins {
            left: -1,
            right: -1,
            top: -1,
            bottom: -1,
        };
        let hr = DwmExtendFrameIntoClientArea(hwnd, &margins);
        if hr.is_err() {
            warn!("DwmExtendFrameIntoClientArea failed: {:?}", hr);
        }

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

        // Tooltip: "GLASS Overlay — Right-click to quit"
        let tip = "GLASS Overlay \u{2014} Right-click to quit\0";
        for (i, ch) in tip.encode_utf16().enumerate() {
            if i >= nid.szTip.len() {
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
