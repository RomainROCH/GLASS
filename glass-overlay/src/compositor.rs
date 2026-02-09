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
use tracing::{info, info_span};
use windows::core::Interface;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::DirectComposition::*;

/// Owns the DirectComposition device, target, and visual.
///
/// Must outlive the wgpu [`Surface`](wgpu::Surface) that references the visual.
/// The visual pointer is passed to wgpu via [`visual_handle`](Self::visual_handle).
pub struct Compositor {
    device: IDCompositionDevice,
    _target: IDCompositionTarget, // prevent drop
    visual: IDCompositionVisual,
}

impl Compositor {
    /// Create a DirectComposition device, target, and visual for `hwnd`.
    ///
    /// The target's root visual is set so wgpu can bind a swapchain to it.
    pub fn new(hwnd: HWND) -> Result<Self, GlassError> {
        unsafe {
            let device: IDCompositionDevice =
                DCompositionCreateDevice(None).map_err(|e| {
                    GlassError::CompositionInit(format!("DCompositionCreateDevice: {e}"))
                })?;

            let target = device.CreateTargetForHwnd(hwnd, true).map_err(|e| {
                GlassError::CompositionInit(format!("CreateTargetForHwnd: {e}"))
            })?;

            let visual = device.CreateVisual().map_err(|e| {
                GlassError::CompositionInit(format!("CreateVisual: {e}"))
            })?;

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

    /// Get the visual pointer for [`wgpu::SurfaceTargetUnsafe::CompositionVisual`].
    pub fn visual_handle(&self) -> NonNull<c_void> {
        NonNull::new(self.visual.as_raw()).expect("IDCompositionVisual pointer is null")
    }

    /// Commit pending changes. Must be called after wgpu configures the surface
    /// so the swapchain binding (`SetContent`) takes effect.
    pub fn commit(&self) -> Result<(), GlassError> {
        let _span = info_span!("dcomp_commit").entered();
        unsafe {
            self.device
                .Commit()
                .map_err(|e| GlassError::CompositionInit(format!("DComp Commit: {e}")))
        }
    }
}
