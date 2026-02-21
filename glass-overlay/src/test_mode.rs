//! Test build mode constants and helpers.
//!
//! When the `test_mode` Cargo feature is enabled, the overlay renders a
//! permanent watermark, forces input passthrough, and enables verbose logging.
//! Useful for visual validation, demoing, and screenshot/recording sessions.
//!
//! # Usage
//!
//! ```sh
//! cargo build -p glass-starter --features test_mode
//! ```

/// Whether the current build has test mode enabled.
#[cfg(feature = "test_mode")]
pub const TEST_MODE: bool = true;
#[cfg(not(feature = "test_mode"))]
pub const TEST_MODE: bool = false;

/// Watermark text lines rendered in the bottom-right corner.
#[cfg(feature = "test_mode")]
pub const WATERMARK_LINES: &[&str] = &[
    "GLASS — Windows Overlay Framework",
    "github.com/user/GLASS-UltimateOverlay",
    "[test_mode build]",
];

/// Font size for the watermark (logical pixels).
#[cfg(feature = "test_mode")]
pub const WATERMARK_FONT_SIZE: f32 = 14.0;

/// When true, input passthrough is forced regardless of config.
/// Interactive hotkeys are ignored in test mode.
#[cfg(feature = "test_mode")]
pub const FORCE_INPUT_PASSTHROUGH: bool = true;
#[cfg(not(feature = "test_mode"))]
pub const FORCE_INPUT_PASSTHROUGH: bool = false;

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
