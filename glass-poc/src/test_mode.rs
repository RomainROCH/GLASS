//! Test build mode constants for the PoC harness.
//!
//! Enables a visible watermark, forces passthrough behavior, and
//! bumps logging to TRACE during anti-cheat validation.

/// Whether the current build has test mode enabled.
#[cfg(feature = "test_mode")]
pub const TEST_MODE: bool = true;
#[cfg(not(feature = "test_mode"))]
pub const TEST_MODE: bool = false;

/// Window title prefix for test mode builds.
#[cfg(feature = "test_mode")]
pub const TITLE_PREFIX: &str = "[MODE TEST] ";
#[cfg(not(feature = "test_mode"))]
pub const TITLE_PREFIX: &str = "";

/// Tray tooltip for test mode.
#[cfg(feature = "test_mode")]
pub const TRAY_TOOLTIP: &str = "[MODE TEST] GLASS Overlay \u{2014} Right-click to quit";
#[cfg(not(feature = "test_mode"))]
pub const TRAY_TOOLTIP: &str = "GLASS Overlay \u{2014} Right-click to quit";

/// Watermark color (premultiplied RGBA).
#[cfg(feature = "test_mode")]
pub const WATERMARK_COLOR: [f32; 4] = [1.0 * 0.35, 1.0 * 0.35, 1.0 * 0.35, 0.35];
