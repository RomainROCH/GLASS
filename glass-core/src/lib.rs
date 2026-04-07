//! Foundational types shared across the GLASS overlay framework.
//!
//! `glass-core` is the minimal, dependency-light base crate that every GLASS
//! crate depends on.  It currently houses:
//!
//! * [`GlassError`] — the single top-level error type returned by all
//!   fallible GLASS operations (DirectComposition, wgpu, Win32 window
//!   creation, HDR detection, config loading, and input subsystem errors).
//!
//! Keeping error types in a separate crate avoids circular dependencies
//! between `glass-overlay` and any downstream crate that needs to match on
//! `GlassError` without pulling in the full overlay stack.

#![warn(missing_docs)]

/// Error types used throughout the GLASS framework.
///
/// Re-exports [`GlassError`] as the crate's top-level error type.
pub mod error;

pub use error::GlassError;
