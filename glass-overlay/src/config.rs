//! Hot-reloadable configuration for the GLASS overlay.
//!
//! Supports RON and TOML formats, detected by file extension.
//! Uses `notify` for filesystem watching and `arc-swap` for lock-free
//! reads from the render loop.
//!
//! # Usage
//! ```no_run
//! use glass_overlay::config::ConfigStore;
//! let store = ConfigStore::load("config.ron").unwrap();
//! // Start watching for changes (spawns a background thread)
//! store.watch().unwrap();
//! // Read from hot path (lock-free, zero allocations)
//! let cfg = store.get();
//! println!("opacity = {}", cfg.opacity);
//! ```

use crate::layout::LayoutConfig;
use crate::modules::ModulesConfig;
use arc_swap::ArcSwap;
use glass_core::GlassError;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// RGBA colour expressed as four `f32` components in `[0.0, 1.0]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rgba(pub f32, pub f32, pub f32, pub f32);

impl Default for Rgba {
    fn default() -> Self {
        Self(1.0, 1.0, 1.0, 1.0)
    }
}

/// 2D position in logical (DPI-independent) pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    /// Horizontal coordinate in logical pixels.
    pub x: f32,
    /// Vertical coordinate in logical pixels.
    pub y: f32,
}

impl Default for Position {
    fn default() -> Self {
        Self { x: 20.0, y: 20.0 }
    }
}

/// 2D size in logical (DPI-independent) pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Size {
    /// Width in logical pixels.
    pub width: f32,
    /// Height in logical pixels.
    pub height: f32,
}

impl Default for Size {
    fn default() -> Self {
        Self {
            width: 360.0,
            height: 60.0,
        }
    }
}

/// Colour palette for overlay elements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Colors {
    /// Background / primary colour.
    pub primary: Rgba,
    /// Text / secondary colour.
    pub secondary: Rgba,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            primary: Rgba(0.0, 0.0, 0.0, 0.6),
            secondary: Rgba(1.0, 1.0, 1.0, 1.0),
        }
    }
}

/// Input/interaction configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputConfig {
    /// Virtual key code for the interactive-mode toggle hotkey.
    /// Default: `0x7B` = F12. Common alternatives: `0x79` (F10), `0x7A` (F11).
    #[serde(default = "default_hotkey_vk")]
    pub hotkey_vk: u32,
    /// Hotkey modifier flags (Win32 `MOD_*` bit mask).
    /// Default: `0` (no modifier). `1` = Alt, `2` = Ctrl, `4` = Shift, `8` = Win.
    #[serde(default)]
    pub hotkey_modifiers: u32,
    /// Interactive mode timeout in milliseconds now configurable.
    /// After this duration, the overlay reverts to passive mode.
    #[serde(default = "default_interactive_timeout_ms")]
    pub interactive_timeout_ms: u32,
    /// Whether to show the visual indicator (border + label) in interactive mode.
    #[serde(default = "default_show_indicator")]
    pub show_indicator: bool,
}

fn default_hotkey_vk() -> u32 {
    0x7B // VK_F12
}

fn default_interactive_timeout_ms() -> u32 {
    4000
}

fn default_show_indicator() -> bool {
    true
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            hotkey_vk: default_hotkey_vk(),
            hotkey_modifiers: 0,
            interactive_timeout_ms: default_interactive_timeout_ms(),
            show_indicator: default_show_indicator(),
        }
    }
}

/// Root overlay configuration.
///
/// Fields are validated on load — out-of-range values are clamped and logged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverlayConfig {
    /// Initial overlay window position in logical pixels.
    #[serde(default)]
    pub position: Position,
    /// Overlay window size in logical pixels.
    #[serde(default)]
    pub size: Size,
    /// Opacity in `[0.0, 1.0]`. Values outside this range are clamped.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// Colour palette for overlay elements.
    #[serde(default)]
    pub colors: Colors,
    /// Input mode configuration (hotkey, timeout, indicator).
    #[serde(default)]
    pub input: InputConfig,
    /// Module system configuration (enable/disable individual modules).
    #[serde(default)]
    pub modules: ModulesConfig,
    /// Layout system configuration (anchor-based widget positioning).
    #[serde(default)]
    pub layout: LayoutConfig,
}

fn default_opacity() -> f32 {
    1.0
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            position: Position::default(),
            size: Size::default(),
            opacity: 1.0,
            colors: Colors::default(),
            input: InputConfig::default(),
            modules: ModulesConfig::default(),
            layout: LayoutConfig::default(),
        }
    }
}

impl OverlayConfig {
    /// Clamp and validate fields, logging any corrections.
    fn validate(&mut self) {
        if self.opacity < 0.0 || self.opacity > 1.0 {
            warn!(
                "Config: opacity {:.3} out of range [0,1], clamping",
                self.opacity
            );
            self.opacity = self.opacity.clamp(0.0, 1.0);
        }
        if self.size.width <= 0.0 {
            warn!("Config: width {:.1} <= 0, clamping to 1", self.size.width);
            self.size.width = 1.0;
        }
        if self.size.height <= 0.0 {
            warn!(
                "Config: height {:.1} <= 0, clamping to 1",
                self.size.height
            );
            self.size.height = 1.0;
        }
    }

    /// Produce a human-readable summary of differences between two configs.
    fn diff_summary(&self, other: &Self) -> String {
        let mut changes = Vec::new();
        if self.opacity != other.opacity {
            changes.push(format!("opacity: {:.2} -> {:.2}", self.opacity, other.opacity));
        }
        if self.position != other.position {
            changes.push(format!(
                "position: ({:.0},{:.0}) -> ({:.0},{:.0})",
                self.position.x, self.position.y, other.position.x, other.position.y
            ));
        }
        if self.size != other.size {
            changes.push(format!(
                "size: ({:.0}x{:.0}) -> ({:.0}x{:.0})",
                self.size.width, self.size.height, other.size.width, other.size.height
            ));
        }
        if self.colors != other.colors {
            changes.push("colors: changed".to_string());
        }
        if self.input != other.input {
            changes.push("input: changed".to_string());
        }
        if self.modules != other.modules {
            changes.push("modules: changed".to_string());
        }
        if self.layout != other.layout {
            changes.push("layout: changed".to_string());
        }
        if changes.is_empty() {
            "no changes".to_string()
        } else {
            changes.join(", ")
        }
    }
}

/// Detected config file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigFormat {
    Ron,
    Toml,
}

fn detect_format(path: &Path) -> Result<ConfigFormat, GlassError> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("ron") => Ok(ConfigFormat::Ron),
        Some("toml") => Ok(ConfigFormat::Toml),
        Some(ext) => Err(GlassError::ConfigError(format!(
            "Unknown config extension '.{ext}'; expected .ron or .toml"
        ))),
        None => Err(GlassError::ConfigError(
            "Config file has no extension; expected .ron or .toml".into(),
        )),
    }
}

fn parse_config(content: &str, format: ConfigFormat) -> Result<OverlayConfig, GlassError> {
    let mut cfg: OverlayConfig = match format {
        ConfigFormat::Ron => {
            ron::from_str(content).map_err(|e| GlassError::ConfigError(format!("RON parse: {e}")))?
        }
        ConfigFormat::Toml => toml::from_str(content)
            .map_err(|e| GlassError::ConfigError(format!("TOML parse: {e}")))?,
    };
    cfg.validate();
    Ok(cfg)
}

/// Thread-safe config store backed by `ArcSwap`.
///
/// Provides lock-free, allocation-free reads via [`get`](Self::get) and
/// filesystem-watched hot-reload via [`watch`](Self::watch).
pub struct ConfigStore {
    inner: Arc<ArcSwap<OverlayConfig>>,
    path: PathBuf,
    format: ConfigFormat,
    /// Kept alive to keep the watcher thread running.
    _watcher: std::sync::Mutex<Option<RecommendedWatcher>>,
}

impl ConfigStore {
    /// Load config from `path`. Format is detected by extension (`.ron` / `.toml`).
    ///
    /// If the file does not exist, creates it with defaults and logs a message.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, GlassError> {
        let path = path.as_ref().to_path_buf();
        let format = detect_format(&path)?;

        let cfg = if path.exists() {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| GlassError::ConfigError(format!("Read {}: {e}", path.display())))?;
            let cfg = parse_config(&content, format)?;
            info!("Config loaded from {}", path.display());
            cfg
        } else {
            let cfg = OverlayConfig::default();
            // Write default config
            let content = match format {
                ConfigFormat::Ron => {
                    ron::ser::to_string_pretty(&cfg, ron::ser::PrettyConfig::default())
                        .unwrap_or_default()
                }
                ConfigFormat::Toml => toml::to_string_pretty(&cfg).unwrap_or_default(),
            };
            if let Err(e) = std::fs::write(&path, &content) {
                warn!("Could not write default config to {}: {e}", path.display());
            } else {
                info!(
                    "Config file not found; created defaults at {}",
                    path.display()
                );
            }
            cfg
        };

        Ok(Self {
            inner: Arc::new(ArcSwap::from_pointee(cfg)),
            path,
            format,
            _watcher: std::sync::Mutex::new(None),
        })
    }

    /// Get the current config snapshot (lock-free, allocation-free).
    ///
    /// The returned `Arc<OverlayConfig>` is valid even if a reload happens
    /// concurrently — the old value stays alive until all readers drop it.
    pub fn get(&self) -> arc_swap::Guard<Arc<OverlayConfig>> {
        self.inner.load()
    }

    /// Start watching the config file for changes. Spawns a background thread.
    ///
    /// On successful reload, the new config is swapped in atomically.
    /// On parse failure, the previous config is kept and an error is logged.
    pub fn watch(&self) -> Result<(), GlassError> {
        let inner = Arc::clone(&self.inner);
        let path = self.path.clone();
        let format = self.format;

        // We need to watch the parent directory because some editors
        // do atomic saves (write to temp + rename), which may not trigger
        // a modify event on the file itself.
        let watch_dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let file_name = path
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default();

        let mut watcher = notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    use notify::EventKind::*;
                    match event.kind {
                        Modify(_) | Create(_) => {
                            // Only react to our config file
                            let is_our_file = event.paths.iter().any(|p| {
                                p.file_name()
                                    .map(|n| n == file_name)
                                    .unwrap_or(false)
                            });
                            if !is_our_file {
                                return;
                            }

                            // Brief debounce — some editors trigger multiple events
                            std::thread::sleep(Duration::from_millis(50));

                            let target = event
                                .paths
                                .iter()
                                .find(|p| {
                                    p.file_name()
                                        .map(|n| n == file_name)
                                        .unwrap_or(false)
                                })
                                .cloned()
                                .unwrap_or_else(|| path.clone());

                            match std::fs::read_to_string(&target) {
                                Ok(content) => match parse_config(&content, format) {
                                    Ok(new_cfg) => {
                                        let old = inner.load();
                                        let diff = old.diff_summary(&new_cfg);
                                        inner.store(Arc::new(new_cfg));
                                        info!("Config reloaded: {diff}");
                                    }
                                    Err(e) => {
                                        error!(
                                            "Config reload parse error (keeping previous): {e}"
                                        );
                                    }
                                },
                                Err(e) => {
                                    warn!("Config reload read error: {e}");
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    error!("Config watcher error: {e}");
                }
            },
        )
        .map_err(|e| GlassError::ConfigError(format!("Watcher init: {e}")))?;

        watcher
            .watch(&watch_dir, RecursiveMode::NonRecursive)
            .map_err(|e| GlassError::ConfigError(format!("Watch {}: {e}", watch_dir.display())))?;

        info!("Config watcher active on {}", self.path.display());

        // Store watcher to keep it alive
        if let Ok(mut guard) = self._watcher.lock() {
            *guard = Some(watcher);
        }

        Ok(())
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_config (RON) ───────────────────────────────────────────────

    #[test]
    fn parse_empty_ron_struct_yields_defaults() {
        // "()" is the RON representation of a struct with all default fields.
        let cfg = parse_config("()", ConfigFormat::Ron).unwrap();
        assert_eq!(cfg.opacity, default_opacity());
        assert_eq!(cfg.position, Position::default());
        assert_eq!(cfg.size, Size::default());
    }

    #[test]
    fn parse_ron_explicit_opacity() {
        let ron_str = "(opacity: 0.75)";
        let cfg = parse_config(ron_str, ConfigFormat::Ron).unwrap();
        assert_eq!(cfg.opacity, 0.75);
        // Other fields must still be defaults
        assert_eq!(cfg.position, Position::default());
    }

    #[test]
    fn parse_ron_explicit_position() {
        let ron_str = "(position: (x: 50.0, y: 100.0))";
        let cfg = parse_config(ron_str, ConfigFormat::Ron).unwrap();
        assert_eq!(cfg.position.x, 50.0);
        assert_eq!(cfg.position.y, 100.0);
    }

    #[test]
    fn malformed_ron_returns_error_not_panic() {
        let result = parse_config("{ NOT VALID RON !!!", ConfigFormat::Ron);
        assert!(result.is_err(), "malformed RON should return Err, not panic");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("RON") || err_msg.contains("Config"),
            "error message should indicate RON parse problem: {err_msg}"
        );
    }

    #[test]
    fn malformed_toml_returns_error_not_panic() {
        let result = parse_config("[[[[not toml", ConfigFormat::Toml);
        assert!(result.is_err(), "malformed TOML should return Err");
    }

    // ── validate() ──────────────────────────────────────────────────────

    #[test]
    fn opacity_above_one_is_clamped_to_one() {
        let ron_str = "(opacity: 1.5)";
        let cfg = parse_config(ron_str, ConfigFormat::Ron).unwrap();
        assert_eq!(cfg.opacity, 1.0, "opacity 1.5 should be clamped to 1.0");
    }

    #[test]
    fn negative_opacity_is_clamped_to_zero() {
        let ron_str = "(opacity: -0.5)";
        let cfg = parse_config(ron_str, ConfigFormat::Ron).unwrap();
        assert_eq!(cfg.opacity, 0.0, "negative opacity should be clamped to 0.0");
    }

    #[test]
    fn zero_width_is_clamped_to_one() {
        let ron_str = "(size: (width: 0.0, height: 200.0))";
        let cfg = parse_config(ron_str, ConfigFormat::Ron).unwrap();
        assert_eq!(cfg.size.width, 1.0, "zero width should be clamped to 1.0");
        assert_eq!(cfg.size.height, 200.0, "height should be unchanged");
    }

    #[test]
    fn negative_height_is_clamped_to_one() {
        let ron_str = "(size: (width: 300.0, height: -5.0))";
        let cfg = parse_config(ron_str, ConfigFormat::Ron).unwrap();
        assert_eq!(cfg.size.height, 1.0, "negative height should be clamped to 1.0");
    }

    #[test]
    fn valid_opacity_is_not_clamped() {
        let ron_str = "(opacity: 0.0)";
        let cfg = parse_config(ron_str, ConfigFormat::Ron).unwrap();
        assert_eq!(cfg.opacity, 0.0);
    }

    // ── RON round-trip ───────────────────────────────────────────────────

    #[test]
    fn overlay_config_ron_roundtrip() {
        let original = OverlayConfig {
            opacity: 0.8,
            position: Position { x: 120.0, y: 80.0 },
            size: Size {
                width: 400.0,
                height: 80.0,
            },
            ..OverlayConfig::default()
        };
        let ron_str =
            ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default()).unwrap();
        let parsed = parse_config(&ron_str, ConfigFormat::Ron).unwrap();
        assert_eq!(original, parsed, "round-trip should produce identical config");
    }

    // ── diff_summary ─────────────────────────────────────────────────────

    #[test]
    fn diff_summary_no_changes() {
        let a = OverlayConfig::default();
        let b = OverlayConfig::default();
        assert_eq!(a.diff_summary(&b), "no changes");
    }

    #[test]
    fn diff_summary_reports_opacity_change() {
        let a = OverlayConfig::default();
        let mut b = OverlayConfig::default();
        b.opacity = 0.5;
        let diff = a.diff_summary(&b);
        assert!(diff.contains("opacity"), "diff should mention 'opacity': {diff}");
    }

    #[test]
    fn diff_summary_reports_position_change() {
        let a = OverlayConfig::default();
        let mut b = OverlayConfig::default();
        b.position.x = 999.0;
        let diff = a.diff_summary(&b);
        assert!(diff.contains("position"), "diff should mention 'position': {diff}");
    }

    #[test]
    fn diff_summary_reports_size_change() {
        let a = OverlayConfig::default();
        let mut b = OverlayConfig::default();
        b.size.width = 999.0;
        let diff = a.diff_summary(&b);
        assert!(diff.contains("size"), "diff should mention 'size': {diff}");
    }

    // ── detect_format ────────────────────────────────────────────────────

    #[test]
    fn detect_format_ron_extension() {
        assert_eq!(
            detect_format(std::path::Path::new("config.ron")).unwrap(),
            ConfigFormat::Ron
        );
    }

    #[test]
    fn detect_format_toml_extension() {
        assert_eq!(
            detect_format(std::path::Path::new("overlay.toml")).unwrap(),
            ConfigFormat::Toml
        );
    }

    #[test]
    fn detect_format_unknown_extension_returns_error() {
        let result = detect_format(std::path::Path::new("config.yaml"));
        assert!(result.is_err(), "unknown extension should return Err");
    }

    #[test]
    fn detect_format_no_extension_returns_error() {
        let result = detect_format(std::path::Path::new("config"));
        assert!(result.is_err(), "missing extension should return Err");
    }
}
