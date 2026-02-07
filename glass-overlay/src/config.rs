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
    pub x: f32,
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
    pub width: f32,
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

/// Root overlay configuration.
///
/// Fields are validated on load — out-of-range values are clamped and logged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverlayConfig {
    #[serde(default)]
    pub position: Position,
    #[serde(default)]
    pub size: Size,
    /// Opacity in `[0.0, 1.0]`. Values outside this range are clamped.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default)]
    pub colors: Colors,
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
