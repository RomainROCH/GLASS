//! GLASS Overlay — production overlay library.
//!
//! Provides the core types for creating a transparent, click-through,
//! DirectComposition-based DX12 overlay on Windows:
//!
//! - [`OverlayWindow`](overlay_window) — HWND creation, DPI, tray icon, message pump
//! - [`Compositor`](compositor::Compositor) — DirectComposition device/target/visual
//! - [`Renderer`](renderer::Renderer) — wgpu DX12 rendering backend
//! - [`test_mode`] — Test build mode constants (watermark, forced passthrough)

pub mod compositor;
pub mod config;
pub mod overlay_window;
pub mod renderer;
pub mod test_mode;
