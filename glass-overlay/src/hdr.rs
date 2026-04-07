//! HDR detection and SDR fallback.
//!
//! Uses `IDXGIOutput6` to detect whether the primary display supports
//! HDR (scRGB / ST.2084). Falls back to explicit SDR path when HDR
//! is unavailable or when `--force-sdr` is set.

// # Unsafe usage in this module
//
// - `detect_primary_hdr`: DXGI COM/FFI — `CreateDXGIFactory1`, `EnumAdapters1`,
//   `EnumOutputs`, `GetDesc`, `cast::<IDXGIOutput6>`, and `GetDesc1` are all unsafe
//   COM interface calls. Each interface pointer is returned by the prior call and
//   validated before use via early-return error handling.

use tracing::{info, warn};
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

/// Detected display capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayCapability {
    /// HDR-capable display (scRGB or ST.2084).
    Hdr,
    /// SDR-only display.
    Sdr,
    /// Could not determine (fallback to SDR).
    Unknown,
}

/// Result of HDR detection for a specific output.
#[derive(Debug, Clone)]
pub struct HdrDetectionResult {
    /// Detected display capability (HDR, SDR, or Unknown).
    pub capability: DisplayCapability,
    /// Human-readable color space string reported by DXGI (e.g. `"DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020"`).
    pub color_space: String,
    /// Display output device name as reported by DXGI (e.g. `"\\\\.\\DISPLAY1"`).
    pub output_name: String,
}

/// Detect HDR capability on the primary display.
///
/// Returns `Hdr` if the primary output supports scRGB or ST.2084,
/// `Sdr` if it explicitly doesn't, or `Unknown` if detection fails.
pub fn detect_primary_hdr() -> HdrDetectionResult {
    // SAFETY: All calls in this block are DXGI COM interface calls via the `windows` crate.
    // - `CreateDXGIFactory1` has no external preconditions beyond a D3D-capable Windows
    //   installation; it returns a COM error rather than exhibiting UB if unavailable.
    // - Each subsequent COM call (`EnumAdapters1`, `EnumOutputs`, `GetDesc`, `cast`,
    //   `GetDesc1`) operates on an interface pointer obtained from the immediately
    //   preceding successful call. Early-return error handling ensures no call receives
    //   a null or invalid interface pointer.
    // - All COM objects use the `windows` crate RAII wrappers that call `Release` on drop,
    //   so there are no manual `AddRef`/`Release` calls and no lifetime mismatches.
    unsafe {
        let factory = match CreateDXGIFactory1::<IDXGIFactory1>() {
            Ok(f) => f,
            Err(e) => {
                warn!("HDR detection: CreateDXGIFactory1 failed: {e}");
                return HdrDetectionResult {
                    capability: DisplayCapability::Unknown,
                    color_space: "unknown".into(),
                    output_name: "unknown".into(),
                };
            }
        };

        // Enumerate first adapter, first output
        let adapter = match factory.EnumAdapters1(0) {
            Ok(a) => a,
            Err(e) => {
                warn!("HDR detection: no adapters: {e}");
                return HdrDetectionResult {
                    capability: DisplayCapability::Unknown,
                    color_space: "unknown".into(),
                    output_name: "unknown".into(),
                };
            }
        };

        let output = match adapter.EnumOutputs(0) {
            Ok(o) => o,
            Err(e) => {
                warn!("HDR detection: no outputs: {e}");
                return HdrDetectionResult {
                    capability: DisplayCapability::Unknown,
                    color_space: "unknown".into(),
                    output_name: "unknown".into(),
                };
            }
        };

        let output_name = output
            .GetDesc()
            .map(|d| {
                String::from_utf16_lossy(
                    &d.DeviceName[..d
                        .DeviceName
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(d.DeviceName.len())],
                )
            })
            .unwrap_or_else(|_| "unknown".into());

        // Try IDXGIOutput6 for advanced color space info
        match output.cast::<IDXGIOutput6>() {
            Ok(output6) => match output6.GetDesc1() {
                Ok(desc1) => {
                    let cs = desc1.ColorSpace;
                    let cs_str = format!("{cs:?}");

                    let is_hdr = cs == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
                        || cs == DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709;

                    let cap = if is_hdr {
                        DisplayCapability::Hdr
                    } else {
                        DisplayCapability::Sdr
                    };

                    info!(
                        "HDR detection: output={output_name}, color_space={cs_str}, capability={cap:?}"
                    );

                    HdrDetectionResult {
                        capability: cap,
                        color_space: cs_str,
                        output_name,
                    }
                }
                Err(e) => {
                    warn!("HDR detection: GetDesc1 failed: {e}");
                    HdrDetectionResult {
                        capability: DisplayCapability::Unknown,
                        color_space: "unknown".into(),
                        output_name,
                    }
                }
            },
            Err(_) => {
                info!("HDR detection: IDXGIOutput6 not available (pre-1803?)");
                HdrDetectionResult {
                    capability: DisplayCapability::Sdr,
                    color_space: "sRGB (no IDXGIOutput6)".into(),
                    output_name,
                }
            }
        }
    }
}

/// Choose the preferred wgpu texture format based on HDR capability.
///
/// Returns `(format, pipeline_name)`.
pub fn choose_surface_format(
    capabilities: &[wgpu::TextureFormat],
    hdr: DisplayCapability,
    force_sdr: bool,
) -> (wgpu::TextureFormat, &'static str) {
    // Guard: empty capabilities list — return a safe default
    if capabilities.is_empty() {
        warn!("Surface capabilities list is empty; defaulting to Bgra8UnormSrgb");
        return (wgpu::TextureFormat::Bgra8UnormSrgb, "SDR/sRGB (default)");
    }

    if !force_sdr && hdr == DisplayCapability::Hdr {
        // Prefer Rgba16Float for scRGB HDR
        if capabilities.contains(&wgpu::TextureFormat::Rgba16Float) {
            info!("Selecting HDR/scRGB pipeline (Rgba16Float)");
            return (wgpu::TextureFormat::Rgba16Float, "HDR/scRGB");
        }
        warn!("HDR capable but Rgba16Float not in surface capabilities; falling back to SDR");
    }

    // SDR path: prefer Bgra8UnormSrgb
    let format = if capabilities.contains(&wgpu::TextureFormat::Bgra8UnormSrgb) {
        wgpu::TextureFormat::Bgra8UnormSrgb
    } else {
        capabilities[0]
    };

    info!("Selecting SDR pipeline ({format:?})");
    (format, "SDR/sRGB")
}
