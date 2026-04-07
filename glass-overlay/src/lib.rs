#![warn(missing_docs)]
//! GLASS Overlay — the main library for building Windows overlays with GLASS.
//!
//! Most applications should start from the crate root. The primary app-facing API is:
//!
//! - [`ConfigStore`] / [`OverlayConfig`] for loading and hot-reloading config
//! - [`Compositor`] and [`Renderer`] for DirectComposition + rendering setup
//! - [`LayoutManager`] and [`WidgetWrapper`] for positioning overlay modules
//! - [`ClockModule`], [`SystemStatsModule`], and [`FpsCounterModule`] for built-in widgets
//! - [`InputManager`] and [`InputMode`] for passive vs interactive overlay behavior
//! - [`Scene`] and related scene types when you need direct scene-graph access
//! - [`GlassError`] as the shared error type
//!
//! A typical application wires together:
//!
//! 1. config loading via [`ConfigStore`]
//! 2. window setup through [`overlay_window`]
//! 3. GPU/runtime setup with [`Compositor`] and [`Renderer`]
//! 4. widget/module placement with [`LayoutManager`] and [`WidgetWrapper`]
//! 5. input mode handling with [`InputManager`]
//!
//! # Recommended consumer API
//!
//! Prefer the crate-root re-exports when importing common types:
//!
//! ```no_run
//! use glass_overlay::{
//!     ClockModule, Compositor, ConfigStore, InputManager, LayoutManager, Renderer,
//!     SystemStatsModule, WidgetWrapper,
//! };
//! ```
//!
//! # Lower-level / advanced modules
//!
//! These modules remain available for consumers who need finer control:
//!
//! - [`config`] for the full configuration API
//! - [`input`] for lower-level input and hit-testing primitives
//! - [`layout`] for anchor resolution and manual layout control
//! - [`modules`] for manual module registration via [`modules::ModuleRegistry`]
//! - [`scene`] for direct retained scene-graph manipulation
//! - [`compositor`], [`renderer`], and [`overlay_window`] for lower-level runtime setup
//! - [`safety`] for gaming-specific anti-cheat checks when the feature is enabled

pub mod compositor;
pub mod config;
mod hdr;
pub mod input;
pub mod layout;
pub mod modules;
pub mod overlay_window;
pub mod renderer;
#[cfg(feature = "gaming")]
pub mod safety;
pub mod scene;
mod test_mode;
mod text_renderer;

pub use crate::compositor::Compositor;
pub use crate::config::{Colors, ConfigStore, InputConfig, OverlayConfig, Position, Rgba, Size};
pub use crate::input::{HitTester, InputManager, InputMode, InteractiveRect};
pub use crate::layout::{
    Anchor, BoundingBox, LayoutConfig, LayoutManager, Widget, WidgetLayoutConfig, WidgetWrapper,
};
pub use crate::modules::{
    ClockModule, FpsCounterModule, ModuleInfo, ModulesConfig, OverlayModule, SystemStatsModule,
};
pub use crate::renderer::Renderer;
pub use crate::scene::{Color, NodeId, RectProps, Scene, SceneNode, TextProps};
pub use glass_core::GlassError;
