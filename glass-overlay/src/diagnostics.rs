//! Diagnostics dump: GPU, OS, DWM, DPI, and display info.
//!
//! On fatal GPU/display errors, call [`DiagnosticsReport::capture`] to collect
//! system state and persist it as a JSON file for triage.

use serde::Serialize;
use std::path::Path;
use tracing::{error, info};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::UI::HiDpi::*;
use windows::core::Interface;

/// Complete diagnostics snapshot.
#[derive(Debug, Serialize)]
pub struct DiagnosticsReport {
    pub timestamp: String,
    pub os: OsInfo,
    pub gpu: GpuInfo,
    pub dwm: DwmInfo,
    pub outputs: Vec<OutputInfo>,
    pub process: ProcessInfo,
    pub active_color_pipeline: String,
    pub error_context: Option<String>,
}

/// Windows version information.
#[derive(Debug, Serialize)]
pub struct OsInfo {
    pub product_name: String,
    pub build: u32,
}

/// GPU adapter information.
#[derive(Debug, Serialize)]
pub struct GpuInfo {
    pub vendor_id: u32,
    pub device_id: u32,
    pub description: String,
    pub backend: String,
}

/// Desktop Window Manager state.
#[derive(Debug, Serialize)]
pub struct DwmInfo {
    pub composition_enabled: bool,
}

/// Display output information.
#[derive(Debug, Serialize)]
pub struct OutputInfo {
    pub name: String,
    pub resolution: String,
    pub hdr_capable: bool,
    pub color_space: String,
}

/// Process DPI information.
#[derive(Debug, Serialize)]
pub struct ProcessInfo {
    pub dpi_awareness: String,
    pub effective_dpi: u32,
}

impl DiagnosticsReport {
    /// Capture a full diagnostics snapshot.
    ///
    /// `error_context` describes the triggering error (e.g. `"DXGI_ERROR_DEVICE_REMOVED"`).
    /// `color_pipeline` is the active pipeline (e.g. `"SDR/sRGB"` or `"HDR/scRGB"`).
    pub fn capture(error_context: Option<String>, color_pipeline: &str) -> Self {
        Self {
            timestamp: chrono_like_now(),
            os: OsInfo::capture(),
            gpu: GpuInfo::capture(),
            dwm: DwmInfo::capture(),
            outputs: OutputInfo::enumerate(),
            process: ProcessInfo::capture(),
            active_color_pipeline: color_pipeline.to_string(),
            error_context,
        }
    }

    /// Persist the report to a JSON file.
    ///
    /// Default location: `glass-diagnostics-<timestamp>.json` in the current directory.
    pub fn persist(&self, dir: &Path) -> std::io::Result<std::path::PathBuf> {
        let safe_ts = self.timestamp.replace(':', "-").replace(' ', "_");
        let filename = format!("glass-diagnostics-{safe_ts}.json");
        let path = dir.join(&filename);

        let json = serde_json::to_string_pretty(self).unwrap_or_else(|e| {
            format!("{{\"error\": \"serialization failed: {e}\"}}")
        });

        std::fs::write(&path, &json)?;
        info!("Diagnostics dump written to {}", path.display());
        Ok(path)
    }

    /// Capture and persist in one call, logging any errors.
    pub fn dump(error_context: Option<String>, color_pipeline: &str) {
        let report = Self::capture(error_context, color_pipeline);
        if let Err(e) = report.persist(Path::new(".")) {
            error!("Failed to write diagnostics dump: {e}");
        }
    }
}

impl OsInfo {
    fn capture() -> Self {
        let build = unsafe {
            let mut info: windows::Win32::System::SystemInformation::OSVERSIONINFOW =
                std::mem::zeroed();
            info.dwOSVersionInfoSize = std::mem::size_of::<
                windows::Win32::System::SystemInformation::OSVERSIONINFOW,
            >() as u32;
            #[allow(deprecated)]
            let _ = windows::Win32::System::SystemInformation::GetVersionExW(&mut info);
            info.dwBuildNumber
        };

        Self {
            product_name: "Windows".to_string(),
            build,
        }
    }
}

impl GpuInfo {
    fn capture() -> Self {
        // Try to enumerate via DXGI factory
        unsafe {
            if let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() {
                if let Ok(adapter) = factory.EnumAdapters1(0) {
                    if let Ok(desc) = adapter.GetDesc1() {
                        let name = String::from_utf16_lossy(
                            &desc.Description[..desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len())]
                        );
                        return Self {
                            vendor_id: desc.VendorId,
                            device_id: desc.DeviceId,
                            description: name,
                            backend: "DX12".to_string(),
                        };
                    }
                }
            }
        }

        Self {
            vendor_id: 0,
            device_id: 0,
            description: "unknown".to_string(),
            backend: "DX12".to_string(),
        }
    }
}

impl DwmInfo {
    fn capture() -> Self {
        let composition_enabled = unsafe {
            DwmIsCompositionEnabled().map(|b| b.as_bool()).unwrap_or(false)
        };

        Self {
            composition_enabled,
        }
    }
}

impl OutputInfo {
    fn enumerate() -> Vec<Self> {
        let mut outputs = Vec::new();

        unsafe {
            let factory = match CreateDXGIFactory1::<IDXGIFactory1>() {
                Ok(f) => f,
                Err(_) => return outputs,
            };

            let mut adapter_idx = 0u32;
            while let Ok(adapter) = factory.EnumAdapters1(adapter_idx) {
                let mut output_idx = 0u32;
                while let Ok(output) = adapter.EnumOutputs(output_idx) {
                    if let Ok(desc) = output.GetDesc() {
                        let name = String::from_utf16_lossy(
                            &desc.DeviceName[..desc.DeviceName.iter().position(|&c| c == 0).unwrap_or(desc.DeviceName.len())]
                        );

                        let rect = desc.DesktopCoordinates;
                        let w = (rect.right - rect.left).unsigned_abs();
                        let h = (rect.bottom - rect.top).unsigned_abs();

                        // Try to detect HDR via IDXGIOutput6
                        let (hdr_capable, color_space) = Self::detect_hdr(&output);

                        outputs.push(OutputInfo {
                            name,
                            resolution: format!("{w}x{h}"),
                            hdr_capable,
                            color_space,
                        });
                    }
                    output_idx += 1;
                }
                adapter_idx += 1;
            }
        }

        outputs
    }

    fn detect_hdr(output: &IDXGIOutput) -> (bool, String) {
        unsafe {
            // Try to QI for IDXGIOutput6 (Windows 10 1803+)
            if let Ok(output6) = output.cast::<IDXGIOutput6>() {
                if let Ok(desc1) = output6.GetDesc1() {
                    let cs = desc1.ColorSpace;
                    let hdr = cs == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
                        || cs == DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709;
                    return (hdr, format!("{cs:?}"));
                }
            }
        }
        (false, "unknown (no IDXGIOutput6)".to_string())
    }
}

impl ProcessInfo {
    fn capture() -> Self {
        let (awareness_str, dpi) = unsafe {
            let hwnd = windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow();
            let dpi = GetDpiForWindow(hwnd);

            // GetProcessDpiAwareness is the legacy API; we use
            // GetWindowDpiAwarenessContext when available.
            let ctx = GetWindowDpiAwarenessContext(hwnd);
            let awareness = GetAwarenessFromDpiAwarenessContext(ctx);
            let awareness_str = match awareness {
                DPI_AWARENESS_UNAWARE => "unaware",
                DPI_AWARENESS_SYSTEM_AWARE => "system-aware",
                DPI_AWARENESS_PER_MONITOR_AWARE => "per-monitor",
                _ => "unknown",
            };

            (awareness_str.to_string(), dpi)
        };

        Self {
            dpi_awareness: awareness_str,
            effective_dpi: dpi,
        }
    }
}

/// Simple timestamp without chrono dependency.
fn chrono_like_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s-since-epoch", now.as_secs())
}
