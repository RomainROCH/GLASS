//! wgpu DX12 renderer — green triangle with premultiplied 50% alpha.
//!
//! Uses `SurfaceTarget::Visual` (DirectComposition) for true per-pixel
//! alpha transparency. The composition swapchain supports `PreMultiplied`
//! alpha mode, unlike the HWND-based swapchain which only supports `Opaque`.
//!
//! Retained rendering: draws once, re-renders only on explicit invalidation.

use glass_core::GlassError;
use std::ffi::c_void;
use std::ptr::NonNull;
use tracing::{info, info_span};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

/// WGSL shader for a green triangle at 50% alpha (premultiplied).
const SHADER_SRC: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    // Triangle vertices in clip space
    var positions = array<vec2<f32>, 3>(
        vec2<f32>( 0.0,  0.5),   // top center
        vec2<f32>(-0.5, -0.5),   // bottom left
        vec2<f32>( 0.5, -0.5),   // bottom right
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[idx], 0.0, 1.0);

    // Premultiplied alpha: green at 50% alpha
    // RGB = (0, 1, 0) * 0.5 = (0, 0.5, 0), A = 0.5
    out.color = vec4<f32>(0.0, 0.5, 0.0, 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    /// Initialize wgpu DX12 renderer bound to a DirectComposition visual.
    ///
    /// `visual`: pointer to `IDCompositionVisual` — wgpu will call
    ///           `CreateSwapChainForComposition` and `SetContent`.
    /// `hwnd`: the overlay window — used only for `GetClientRect` sizing.
    pub fn new(visual: NonNull<c_void>, hwnd: HWND) -> Result<Self, GlassError> {
        let _span = info_span!("renderer_init").entered();

        // Get window client area size
        let (width, height) = unsafe {
            let mut rect = windows::Win32::Foundation::RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            (
                (rect.right - rect.left).max(1) as u32,
                (rect.bottom - rect.top).max(1) as u32,
            )
        };

        info!("Initializing wgpu DX12 renderer at {width}x{height}");

        // Create wgpu instance — DX12 only
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..Default::default()
        });

        // Create surface from DirectComposition visual.
        // Uses create_surface_unsafe with CompositionVisual target.
        // SAFETY: visual is a valid IDCompositionVisual pointer owned by Compositor.
        // wgpu internally calls CreateSwapChainForComposition + AddRef on the visual.
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CompositionVisual(
                    visual.as_ptr(),
                ))
                .map_err(|e| GlassError::WgpuInit(format!("Surface creation failed: {e}")))?
        };

        // Request adapter
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

        // Request device
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("GLASS PoC Device"),
                ..Default::default()
            },
            None,
        ))
        .map_err(|e| GlassError::WgpuInit(format!("Device request failed: {e}")))?;

        // Configure surface — use alpha mode for transparency
        let caps = surface.get_capabilities(&adapter);
        info!("Surface capabilities: formats={:?}, alpha_modes={:?}", caps.formats, caps.alpha_modes);

        // Prefer Bgra8UnormSrgb, fall back to first available
        let format = if caps.formats.contains(&wgpu::TextureFormat::Bgra8UnormSrgb) {
            wgpu::TextureFormat::Bgra8UnormSrgb
        } else {
            caps.formats[0]
        };

        // Prefer premultiplied alpha, fall back
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Auto) {
            wgpu::CompositeAlphaMode::Auto
        } else {
            caps.alpha_modes[0]
        };

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

        // Create shader + pipeline
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GLASS Triangle Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GLASS Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("GLASS Triangle Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Premultiplied alpha blending
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        info!("Render pipeline created");

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            pipeline,
        })
    }

    /// Handle window resize — reconfigure the surface.
    pub fn resize(&mut self) -> Result<(), GlassError> {
        let _span = info_span!("resize").entered();
        // Re-query actual window size (we're fullscreen overlay so use screen metrics)
        // For PoC, just reconfigure at current config size
        self.surface.configure(&self.device, &self.surface_config);
        Ok(())
    }

    /// Render one frame: clear to transparent, draw green triangle.
    pub fn render(&mut self) -> Result<(), GlassError> {
        let _acquire_span = info_span!("acquire").entered();
        let frame = self
            .surface
            .get_current_texture()
            .map_err(|e| GlassError::WgpuInit(format!("Surface acquire failed: {e}")))?;
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
                        // Clear to fully transparent
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

            rpass.set_pipeline(&self.pipeline);
            rpass.draw(0..3, 0..1);
        }
        drop(_render_span);

        let _present_span = info_span!("present").entered();
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        drop(_present_span);

        Ok(())
    }
}
