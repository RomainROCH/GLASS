//! Glyphon-based text rendering integration for the scene graph.
//!
//! Wraps `glyphon::TextRenderer`, `TextAtlas`, `FontSystem`, and `SwashCache`
//! into a single `TextEngine` that the main [`Renderer`] can call to draw
//! all `SceneNode::Text` nodes in a single pass.
//!
//! # Usage
//! ```ignore
//! let mut text_engine = TextEngine::new(&device, &queue, surface_format);
//! // per-frame:
//! text_engine.prepare(&device, &queue, &scene, width, height);
//! text_engine.render(&mut render_pass);
//! ```
//!
//! [`Renderer`]: crate::renderer::Renderer

use crate::scene::{Scene, SceneNode};
use glyphon::{
    Buffer, Cache, ColorMode, FontSystem, Metrics, Resolution, SwashCache, TextArea, TextAtlas,
    TextBounds, TextRenderer, Viewport,
};
use tracing::{debug, warn};

/// All-in-one text rendering engine.
///
/// Owns the font system, glyph cache, text atlas, and the glyphon text
/// renderer. Designed for single-threaded overlay usage — NOT `Send`/`Sync`.
pub struct TextEngine {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    viewport: Viewport,
    text_renderer: TextRenderer,
    /// Pre-allocated buffer pool — reused across frames to avoid allocations.
    buffer_pool: Vec<Buffer>,
}

impl TextEngine {
    /// Create a new `TextEngine`.
    ///
    /// # Arguments
    /// * `device` — wgpu device
    /// * `queue` — wgpu queue
    /// * `surface_format` — the texture format used by the surface
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::with_color_mode(
            device,
            queue,
            &cache,
            surface_format,
            ColorMode::Accurate,
        );
        let viewport = Viewport::new(device, &cache);
        let text_renderer = TextRenderer::new(
            &mut atlas,
            device,
            wgpu::MultisampleState::default(),
            None, // no depth stencil for overlay
        );

        debug!("TextEngine initialized (format: {surface_format:?})");

        Self {
            font_system,
            swash_cache,
            atlas,
            viewport,
            text_renderer,
            buffer_pool: Vec::new(),
        }
    }

    /// Prepare text nodes from the scene for rendering.
    ///
    /// Collects all `SceneNode::Text` nodes, lays them out via cosmic-text,
    /// and uploads glyphs to the atlas. Must be called before [`render`].
    ///
    /// # Arguments
    /// * `device` — wgpu device
    /// * `queue` — wgpu queue  
    /// * `scene` — the scene containing text nodes
    /// * `width` — surface width in pixels
    /// * `height` — surface height in pixels
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        width: u32,
        height: u32,
    ) {
        // Update viewport resolution
        self.viewport.update(
            queue,
            Resolution {
                width,
                height,
            },
        );

        // Collect text nodes from scene
        let text_nodes: Vec<_> = scene
            .iter()
            .filter_map(|(_, node)| match node {
                SceneNode::Text(props) => Some(props),
                _ => None,
            })
            .collect();

        if text_nodes.is_empty() {
            // No text to render — still prepare with empty areas to clear state
            let _ = self.text_renderer.prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                [],
                &mut self.swash_cache,
            );
            return;
        }

        // Grow buffer pool if needed (reuse across frames)
        while self.buffer_pool.len() < text_nodes.len() {
            let buffer = Buffer::new(
                &mut self.font_system,
                Metrics::new(16.0, 20.0), // default metrics, will be overridden
            );
            self.buffer_pool.push(buffer);
        }

        // Update each buffer with the corresponding text node
        for (i, text_props) in text_nodes.iter().enumerate() {
            let buffer = &mut self.buffer_pool[i];
            let metrics = Metrics::new(text_props.font_size, text_props.font_size * 1.2);
            buffer.set_metrics(&mut self.font_system, metrics);
            buffer.set_size(&mut self.font_system, Some(width as f32), Some(height as f32));
            buffer.set_text(
                &mut self.font_system,
                &text_props.text,
                glyphon::Attrs::new(),
                glyphon::Shaping::Advanced,
            );
            buffer.shape_until_scroll(&mut self.font_system, false);
        }

        // Build TextArea list
        let text_areas: Vec<TextArea<'_>> = text_nodes
            .iter()
            .enumerate()
            .map(|(i, text_props)| {
                let c = text_props.color;
                TextArea {
                    buffer: &self.buffer_pool[i],
                    left: text_props.x,
                    top: text_props.y,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: width as i32,
                        bottom: height as i32,
                    },
                    default_color: glyphon::Color::rgba(
                        (c.r * 255.0) as u8,
                        (c.g * 255.0) as u8,
                        (c.b * 255.0) as u8,
                        (c.a * 255.0) as u8,
                    ),
                    custom_glyphs: &[],
                }
            })
            .collect();

        // Prepare text for rendering
        if let Err(e) = self.text_renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            text_areas,
            &mut self.swash_cache,
        ) {
            warn!("glyphon prepare error: {e:?}");
        }
    }

    /// Render prepared text into the given render pass.
    ///
    /// Must be called inside an active render pass, after [`prepare`].
    pub fn render<'pass>(
        &'pass self,
        pass: &mut wgpu::RenderPass<'pass>,
    ) {
        if let Err(e) = self.text_renderer.render(&self.atlas, &self.viewport, pass) {
            warn!("glyphon render error: {e:?}");
        }
    }

    /// Trim unused atlas allocations. Call periodically (e.g. every 60 frames).
    pub fn trim(&mut self) {
        self.atlas.trim();
    }
}
