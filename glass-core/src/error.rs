use std::fmt;

/// Top-level error type for GLASS.
#[derive(Debug)]
pub enum GlassError {
    /// DirectComposition initialization failed.
    CompositionInit(String),
    /// wgpu surface/device creation failed.
    WgpuInit(String),
    /// Win32 window creation failed.
    WindowCreation(String),
    /// HDR detection failed — falling back to SDR.
    HdrUnavailable(String),
    /// Configuration loading or parsing failed.
    ConfigError(String),
    /// Input subsystem error (hotkey registration, mode switch).
    InputError(String),
    /// Generic OS error with HRESULT.
    OsError(String),
    /// Anti-cheat safety gate blocked startup.
    SafetyBlock(String),
}

impl fmt::Display for GlassError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlassError::CompositionInit(msg) => write!(f, "DirectComposition init failed: {msg}"),
            GlassError::WgpuInit(msg) => write!(f, "wgpu init failed: {msg}"),
            GlassError::WindowCreation(msg) => write!(f, "Window creation failed: {msg}"),
            GlassError::HdrUnavailable(msg) => write!(f, "HDR unavailable (SDR fallback): {msg}"),
            GlassError::ConfigError(msg) => write!(f, "Config error: {msg}"),
            GlassError::InputError(msg) => write!(f, "Input error: {msg}"),
            GlassError::OsError(msg) => write!(f, "OS error: {msg}"),
            GlassError::SafetyBlock(msg) => write!(f, "Anti-cheat safety block: {msg}"),
        }
    }
}

impl std::error::Error for GlassError {}
