//! DirectComposition setup for true per-pixel alpha transparency.
//!
//! HWND-based DX12 swapchains only support `alpha_modes=[Opaque]`.
//! DirectComposition bypasses this: `CreateSwapChainForComposition` supports
//! `DXGI_ALPHA_MODE_PREMULTIPLIED`, giving us real transparency.
//!
//! Flow: DCompDevice → Target(HWND) → Visual → wgpu binds swapchain to Visual.

use glass_core::GlassError;
use std::ffi::c_void;
use std::ptr::NonNull;
use tracing::info;
use windows::core::Interface;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::DirectComposition::*;

/// Owns the DirectComposition device, target, and visual.
/// Must outlive the wgpu `Surface` that references the visual.
pub struct Compositor {
    device: IDCompositionDevice,
    _target: IDCompositionTarget, // prevent drop
    visual: IDCompositionVisual,
}

impl Compositor {
    /// Create DirectComposition device + target + visual for the given HWND.
    pub fn new(hwnd: HWND) -> Result<Self, GlassError> {
        unsafe {
            // Create DComp device (None = default DXGI device)
            let device: IDCompositionDevice =
                DCompositionCreateDevice(None).map_err(|e| {
                    GlassError::CompositionInit(format!("DCompositionCreateDevice: {e}"))
                })?;

            // Create composition target bound to our overlay HWND.
            // topmost=true → visual renders above window content.
            let target = device.CreateTargetForHwnd(hwnd, true).map_err(|e| {
                GlassError::CompositionInit(format!("CreateTargetForHwnd: {e}"))
            })?;

            // Create visual — wgpu will set its content to the swap chain.
            let visual = device.CreateVisual().map_err(|e| {
                GlassError::CompositionInit(format!("CreateVisual: {e}"))
            })?;

            // Set visual as root of the composition target.
            target.SetRoot(&visual).map_err(|e| {
                GlassError::CompositionInit(format!("SetRoot: {e}"))
            })?;

            info!("DirectComposition initialized (device + target + visual)");

            Ok(Self {
                device,
                _target: target,
                visual,
            })
        }
    }

    /// Get the visual pointer for `wgpu::SurfaceTarget::Visual`.
    pub fn visual_handle(&self) -> NonNull<c_void> {
        NonNull::new(self.visual.as_raw()).expect("IDCompositionVisual pointer is null")
    }

    /// Commit pending changes. Must be called after wgpu configures the surface
    /// so the swap chain binding (SetContent) takes effect.
    pub fn commit(&self) -> Result<(), GlassError> {
        unsafe {
            self.device
                .Commit()
                .map_err(|e| GlassError::CompositionInit(format!("DComp Commit: {e}")))
        }
    }
}
