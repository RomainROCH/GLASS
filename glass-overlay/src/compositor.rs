//! DirectComposition setup for true per-pixel alpha transparency.
//!
//! HWND-based DX12 swapchains only support `alpha_modes=[Opaque]`.
//! DirectComposition bypasses this: `CreateSwapChainForComposition` supports
//! `DXGI_ALPHA_MODE_PREMULTIPLIED`, giving us real transparency.
//!
//! Flow: DCompDevice → Target(HWND) → Visual → wgpu binds swapchain to Visual.

// # Unsafe usage in this module
//
// - `Compositor::new`: COM/FFI — `DCompositionCreateDevice`, `CreateTargetForHwnd`,
//   `CreateVisual`, and `SetRoot` are all `windows`-crate COM calls that the API marks
//   unsafe because they cross the FFI boundary and require callers to uphold COM
//   threading and lifetime contracts.
// - `Compositor::commit`: COM/FFI — `IDCompositionDevice::Commit` is an unsafe COM
//   call; safety is guaranteed by `self` owning the device for its lifetime.

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
        // SAFETY: All COM interface calls in this block uphold the following invariants:
        // - `DCompositionCreateDevice(None)` creates a software-backed DComp device with
        //   no external preconditions; it does not dereference `None` as a pointer.
        // - Each subsequent call (`CreateTargetForHwnd`, `CreateVisual`, `SetRoot`) uses
        //   interface pointers obtained from the immediately preceding successful call,
        //   which are valid and AddRef-counted by the `windows` crate RAII wrappers.
        // - `hwnd` is a valid HWND passed from `create_overlay_window`; if invalid,
        //   `CreateTargetForHwnd` will return a COM error rather than exhibit UB.
        // - The `?` propagation ensures no subsequent call proceeds with a null/invalid
        //   interface pointer from a failed prior call.
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
        // SAFETY: `self.device` is a valid `IDCompositionDevice` COM pointer created
        // in `Compositor::new` and kept alive by Rust's ownership of `self`. The device
        // is not released until `Compositor` is dropped, so this call cannot use a
        // dangling interface pointer. `Commit` has no additional preconditions beyond
        // receiving a valid device pointer.
        unsafe {
            self.device
                .Commit()
                .map_err(|e| GlassError::CompositionInit(format!("DComp Commit: {e}")))
        }
    }
}
