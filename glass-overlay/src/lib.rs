//! GLASS Overlay — production overlay library.
//!
//! Provides the core types for creating a transparent, click-through,
//! DirectComposition-based DX12 overlay on Windows:
//!
//! - [`OverlayWindow`](overlay_window) — HWND creation, DPI, tray icon, message pump
//! - [`Compositor`](compositor::Compositor) — DirectComposition device/target/visual
//! - [`Renderer`](renderer::Renderer) — wgpu DX12 rendering backend
//! - [`scene`] — Retained scene graph with dirty-flag tracking
//! - [`text_renderer`] — Glyphon text rendering integration
//! - [`config`] — Hot-reloadable RON/TOML configuration
//! - [`input`] — Passive/interactive mode switching + rect-based hit-testing
//! - [`hdr`] — HDR detection + SDR fallback
//! - [`diagnostics`] — System diagnostics dump on errors
//! - [`test_mode`] — Test build mode constants (watermark, forced passthrough)

pub mod compositor;
pub mod config;
pub mod diagnostics;
pub mod hdr;
pub mod input;
pub mod layout;
pub mod overlay_window;
pub mod renderer;
pub mod modules;
pub mod safety;
pub mod scene;
pub mod test_mode;
pub mod text_renderer;
