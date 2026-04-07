# Changelog

All notable changes to GLASS will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.2.0] - 2026-04-07

### Added
- `SystemStatsModule::set_temp_source()` for consumer-provided temperature data.
- Comprehensive rustdoc coverage for public types in `glass-core` and `glass-overlay`.
- `#![warn(missing_docs)]` enforcement for `glass-core` and `glass-overlay`.
- SAFETY comments for unsafe blocks in `glass-overlay`.
- Expanded test coverage across scene, layout, config, clock, and system stats behavior, including alpha-mode contract coverage and a minimal example smoke check.

### Changed
- `SystemStatsModule` now relies on an injected temperature callback instead of built-in hardware sensor discovery.
- Public API surface was curated to better reflect the intended consumer-facing library boundary.
- Onboarding and configuration-watch documentation were clarified.
- Cargo dependencies and manifest entries were cleaned up across the workspace.

### Fixed
- Transparency alpha-mode handling for composition surfaces, including `PreMultiplied` alpha support for DX12 composition surfaces.
- Minimal example error handling and CI verification.
- App-name configurability in overlay setup.

### Removed
- Built-in WMI/COM-based temperature detection from `glass-overlay`.
- LibreHardwareMonitor, OpenHardwareMonitor, and `sysinfo::Components` temperature-path usage from `SystemStatsModule`.
- Unused Windows feature flags and related Cargo dependencies tied to the removed temperature implementation.

## [0.1.0] - Initial release

### Added
- Core overlay framework: DirectComposition window, software renderer, and scene graph.
- Built-in modules: clock, system stats (CPU/RAM/temp), and FPS counter.
- Hot-reload configuration via RON files.
- Layout system with anchor-based positioning.
- Input handling with hotkey toggle.
