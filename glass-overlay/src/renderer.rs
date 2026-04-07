//! wgpu DX12 renderer — premultiplied-alpha overlay rendering.
//!
//! Uses `SurfaceTargetUnsafe::CompositionVisual` (DirectComposition) for true
//! per-pixel alpha transparency. The composition swapchain supports
//! `PreMultiplied` alpha mode.
//!
//! Retained rendering: draws once, re-renders only on explicit invalidation.
//! Scene graph text is prepared and drawn through an internal glyph rendering
//! engine owned by [`Renderer`].

// # Unsafe usage in this module
//
// - `Renderer::new` (GetClientRect): Win32 FFI — reads the window's client rectangle
//   via a raw HWND; the result may be zeroed on error, handled by `.max(1)` guards.
// - `Renderer::new` (create_surface_unsafe): wgpu FFI — binds a wgpu surface to a
//   raw `IDCompositionVisual*`; the visual must remain valid for the surface lifetime.

use crate::hdr;
use crate::scene::Scene;
use crate::text_renderer::TextEngine;
use glass_core::GlassError;
use std::ffi::c_void;
use std::fmt;
use std::ptr::NonNull;
use tracing::{info, info_span, warn};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

/// Timer ID used for periodic module updates in the message loop.
pub const MODULE_UPDATE_TIMER_ID: usize = 43;

/// DirectComposition-backed surfaces must support premultiplied alpha for GLASS.
fn select_composition_alpha_mode(
    alpha_modes: &[wgpu::CompositeAlphaMode],
) -> Result<wgpu::CompositeAlphaMode, GlassError> {
    if alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
        Ok(wgpu::CompositeAlphaMode::PreMultiplied)
    } else {
        Err(GlassError::WgpuInit(
            "Surface missing CompositeAlphaMode::PreMultiplied".into(),
        ))
    }
}

/// Low-level GPU backend wrapper for wgpu/DX12 draw submission.
///
/// Encapsulates GPU initialization, swapchain presentation, and rendering.
/// All rendering goes through this struct.
///
/// # Thread Safety
/// Not `Send`/`Sync` — must be used on the thread that created it.
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    /// Active color pipeline name for diagnostics.
    color_pipeline: &'static str,
    /// Retained scene graph.
    scene: Scene,
    /// Glyphon text rendering engine.
    text_engine: TextEngine,
}

impl fmt::Debug for Renderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Renderer")
            .field("surface_config", &self.surface_config)
            .field("color_pipeline", &self.color_pipeline)
            .field("scene_nodes", &self.scene.len())
            .field("scene_dirty", &self.scene.is_dirty())
            .finish()
    }
}

impl Renderer {
    /// Initialize the wgpu DX12 renderer bound to a DirectComposition visual.
    ///
    /// # Arguments
    /// * `visual` — pointer to `IDCompositionVisual` from
    ///   [`crate::Compositor::visual_handle`].
    /// * `hwnd` — the overlay window; used only for `GetClientRect` sizing.
    ///
    /// # Errors
    /// Returns [`GlassError::WgpuInit`] if adapter/device/surface creation fails.
    pub fn new(visual: NonNull<c_void>, hwnd: HWND) -> Result<Self, GlassError> {
        let _span = info_span!("renderer_init").entered();

        let (width, height) = {
            // SAFETY: `hwnd` is the overlay window handle created by `create_overlay_window`
            // and passed directly to this function without modification. `rect` is a
            // zero-initialized local `RECT` on the stack — `GetClientRect` writes into it
            // via a valid pointer. On failure (e.g. window not yet fully created), the RECT
            // fields remain zero; the `.max(1)` guards below ensure the surface dimensions
            // are never zero regardless.
            unsafe {
                let mut rect = windows::Win32::Foundation::RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                (
                    (rect.right - rect.left).max(1) as u32,
                    (rect.bottom - rect.top).max(1) as u32,
                )
            }
        };

        info!("Initializing wgpu DX12 renderer at {width}x{height}");

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..Default::default()
        });

        // SAFETY: `visual` is a `NonNull<c_void>` pointing to a live `IDCompositionVisual`
        // obtained from `Compositor::visual_handle`. The `NonNull` invariant guarantees the
        // pointer is non-null. The `Compositor` is held by the caller for the duration of
        // the application's lifetime, which must exceed the lifetime of this `Renderer`
        // (and its enclosed `Surface<'static>`). `SurfaceTargetUnsafe::CompositionVisual`
        // requires exactly this: a non-null `IDCompositionVisual*` that remains valid for
        // as long as the surface exists.
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CompositionVisual(
                    visual.as_ptr(),
                ))
                .map_err(|e| GlassError::WgpuInit(format!("Surface creation failed: {e}")))?
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| GlassError::WgpuInit("No compatible DX12 adapter found".into()))?;

        let adapter_info = adapter.get_info();
        info!(
            "Using GPU: {} (backend: {:?})",
            adapter_info.name, adapter_info.backend
        );

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("GLASS Device"),
                ..Default::default()
            },
            None,
        ))
        .map_err(|e| GlassError::WgpuInit(format!("Device request failed: {e}")))?;

        let caps = surface.get_capabilities(&adapter);
        info!(
            "Surface capabilities: formats={:?}, alpha_modes={:?}",
            caps.formats, caps.alpha_modes
        );

        // HDR detection + format selection
        let hdr_result = hdr::detect_primary_hdr();
        info!(
            "Primary display capability: output={}, color_space={}, capability={:?}",
            hdr_result.output_name, hdr_result.color_space, hdr_result.capability
        );
        let force_sdr = std::env::args().any(|a| a == "--force-sdr");
        if force_sdr {
            info!("--force-sdr flag: forcing SDR pipeline");
        }
        let (format, color_pipeline) =
            hdr::choose_surface_format(&caps.formats, hdr_result.capability, force_sdr);

        let alpha_mode = select_composition_alpha_mode(&caps.alpha_modes)?;

        info!("Using format: {format:?}, alpha_mode: {alpha_mode:?}");

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Mailbox,
            desired_maximum_frame_latency: 1,
            alpha_mode,
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        let text_engine = TextEngine::new(&device, &queue, format);

        #[allow(unused_mut)]
        let mut scene = Scene::new();

        // In test mode, add watermark text nodes using the scene graph.
        #[cfg(feature = "test_mode")]
        {
            use crate::scene::{Color, TextProps};
            use crate::test_mode;
            let wm_x = (width as f32) * 0.55;
            let mut wm_y = (height as f32) - 60.0;
            for line in test_mode::WATERMARK_LINES {
                scene.add_text(TextProps {
                    x: wm_x,
                    y: wm_y,
                    text: (*line).to_string(),
                    font_size: test_mode::WATERMARK_FONT_SIZE,
                    color: Color::new(1.0, 1.0, 1.0, 0.35),
                });
                wm_y += test_mode::WATERMARK_FONT_SIZE * 1.25;
            }
            info!(
                "Test mode: {} watermark text nodes added",
                test_mode::WATERMARK_LINES.len()
            );
        }

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            color_pipeline,
            scene,
            text_engine,
        })
    }

    /// Get the active color pipeline name (for diagnostics).
    pub fn color_pipeline(&self) -> &'static str {
        self.color_pipeline
    }

    /// Handle window resize — reconfigure the surface with new dimensions.
    ///
    /// **P0 fix**: Previous version did not update `surface_config.width/height`
    /// before reconfiguring, causing stale dimensions.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), GlassError> {
        let _span = info_span!("resize").entered();
        let width = width.max(1);
        let height = height.max(1);
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        info!("Surface resized to {width}x{height}");
        Ok(())
    }

    /// Get current surface dimensions.
    pub fn surface_dims(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    /// Render one frame: clear to transparent, then draw scene text.
    ///
    /// Includes surface error recovery: on `Lost` or `Outdated`, reconfigures
    /// the surface and retries once before returning an error.
    pub fn render(&mut self) -> Result<(), GlassError> {
        // Prepare text engine with current scene
        self.text_engine.prepare(
            &self.device,
            &self.queue,
            &self.scene,
            self.surface_config.width,
            self.surface_config.height,
        );

        let _acquire_span = info_span!("acquire").entered();
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                // Surface lost/outdated — reconfigure and retry once
                warn!("Surface lost/outdated — reconfiguring and retrying");
                self.surface.configure(&self.device, &self.surface_config);
                self.surface.get_current_texture().map_err(|e| {
                    let msg = format!("Surface acquire failed after reconfigure: {e}");
                    warn!("Fatal GPU error: {msg}");
                    GlassError::WgpuInit(msg)
                })?
            }
            Err(wgpu::SurfaceError::Timeout) => {
                // Timeout is transient — skip this frame
                warn!("Surface acquire timeout — skipping frame");
                return Ok(());
            }
            Err(e) => {
                let msg = format!("Surface acquire failed: {e}");
                warn!("Fatal GPU error: {msg}");
                return Err(GlassError::WgpuInit(msg));
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        drop(_acquire_span);

        let _render_span = info_span!("render").entered();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GLASS Frame Encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("GLASS Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Draw scene text (watermark in test_mode, future HUD text)
            self.text_engine.render(&mut rpass);
        }
        drop(_render_span);

        let _present_span = info_span!("present").entered();
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        drop(_present_span);

        // Clear dirty flags after successful render
        self.scene.clear_dirty();

        Ok(())
    }

    /// Get a mutable reference to the scene graph.
    ///
    /// External code can add/update/remove scene nodes. Changes will be
    /// rendered on the next `render()` call.
    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    /// Get a read-only reference to the scene graph.
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
}

#[cfg(test)]
mod tests {
    use super::select_composition_alpha_mode;
    use glass_core::GlassError;

    #[test]
    fn selects_premultiplied_alpha_mode_when_supported() {
        let alpha_mode = select_composition_alpha_mode(&[
            wgpu::CompositeAlphaMode::Opaque,
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::PostMultiplied,
        ])
        .expect("premultiplied alpha mode should be selected");

        assert_eq!(alpha_mode, wgpu::CompositeAlphaMode::PreMultiplied);
    }

    #[test]
    fn rejects_surface_without_premultiplied_alpha_mode() {
        let err = select_composition_alpha_mode(&[
            wgpu::CompositeAlphaMode::Opaque,
            wgpu::CompositeAlphaMode::PostMultiplied,
            wgpu::CompositeAlphaMode::Inherit,
        ])
        .expect_err("surface without premultiplied alpha must fail");

        match err {
            GlassError::WgpuInit(msg) => {
                assert_eq!(msg, "Surface missing CompositeAlphaMode::PreMultiplied");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
