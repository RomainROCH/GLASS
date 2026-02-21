# GENERICITY AUDIT — GLASS Overlay Framework

Last updated: 2026-02-21
Author: GLASS contributors (audit produced by automation)

Purpose
-------
This document audits the repository for "genericity" — i.e., how suitable
this codebase is as a general-purpose Windows overlay framework that any
developer can adopt as a secure, high-performance foundation for overlays.
It summarizes what was changed to improve generality, what remains platform-
or use-case-specific, and practical recommendations and an action plan.

Summary of recent changes (relevant to genericity)
-----------------------------------------------
- `glass-poc` renamed to `glass-starter` and reworded as a reference starter
  application rather than a PoC. This clarifies the repo's intent as a
  reusable base.
- Anti-cheat detection logic was feature-gated behind a new `gaming`
  Cargo feature. Default builds are no longer gaming-specific or hostile to
  productivity users.
- WSL2/Windows parity improvements:
  - `/usr/local/bin/cargo` wrapper to invoke Windows `cargo.exe` in non-interactive shells.
  - `C:\Users\<user>\.cargo\link-wrapper.cmd` created to dynamically resolve
    the MSVC `link.exe` and Windows SDK paths (avoids hardcoded MSVC versions).
  - Project-local `.cargo/config.toml` was updated with documentation on the
    WSL2 setup and the default build target (`x86_64-pc-windows-msvc`).
- `config.ron` augmented with `modules` and `layout` sections and inline
  documentation so configuration is complete and self-describing.
- `alloc_tracker.rs` is preserved but annotated with `#![allow(dead_code)]`
  so the default build is warning-clean.
- `README.md`, `CLAUDE.md` and CI workflows updated to reflect the new
  paradigm and to document the WSL2 build/compatibility steps.

Current strong points for generic use
-----------------------------------
- Clear crate separation: `glass-core` (errors), `glass-overlay` (framework),
  `glass-starter` (example/starting app).
- Modular OverlayModule trait and ModuleRegistry pattern — easy to extend
  with new modules for arbitrary overlays.
- Retained scene graph and zero-allocation steady-state rendering — good
  performance characteristics for production overlays.
- Hot-reloadable configuration (RON/TOML) with arc-swap for lock-free reads.
- DX12/DirectComposition-specific work (premultiplied alpha support) to
  enable true per-pixel alpha overlays on Windows.
- CI targeted at Windows and uses MSVC toolchain — aligns with real-world user environment.

Remaining gaps and portability pain points
----------------------------------------
1. Platform abstraction: nearly all overlay/window code is Windows-only and
   directly uses Win32 / DirectComposition APIs. There is no platform trait
   layer to allow alternate backends (e.g., macOS, Linux, or cross-platform
   windowing through `winit`).

2. Rendering backend coupling: the code assumes a DX12-only wgpu backend.
   The workspace patches a forked `wgpu-hal`/`wgpu-types`/`naga` to support
   DirectComposition. This is correct for the Windows goal but makes upstream
   upgrades and cross-backend support harder.

3. Build/tooling fragility: the WSL2 helper wrappers work but are platform-
   specific and machine-local; contributors will need to perform local setup
   or use the wrapper scripts. No containerized or reproducible build path is
   provided for non-Windows development.

4. Documentation & examples: README and starter are present but more
   examples showing how to implement common overlays (notifications, HUDs,
   click-through interactive widgets) would help adoption.

5. Tests & CI: unit tests exist for module registry; however integration tests
   that exercise the overlay with a headless render or smoke tests on
   Windows are absent. The CI only runs on `windows-latest` — no coverage for
   cross-compiles or static analysis on Linux.

6. Packaging & API stability: no semantic versioning policy or CHANGELOG.
   Public API surface (OverlayModule trait, Config structs) should be
   documented and stabilized if this is intended as a reusable library.

7. Security & capability model: while the anti-cheat code is read-only, the
   repository lacks an explicit security policy (what is allowed, what is
   forbidden) and contributor guidance for maintaining privacy and safety.

Recommended work to maximize genericity (prioritized)
----------------------------------------------------
Short-term (low-effort, high-impact)
- 1. Document contribution and release process: add `CONTRIBUTING.md` and
     a `CHANGELOG.md` with semantic versioning guidance.
- 2. Add `CODE_OF_CONDUCT.md` and a short `SECURITY.md` describing the
     project's security posture and reporting process.
- 3. Add a `docs/` index with usage recipes (how to add a module, how to
     localize, theming, accessibility guidance).
- 4. Add minimal examples under `examples/` (or `glass-starter/examples/`) —
     e.g., notification overlay, click-through HUD, interactive widget demo.
- 5. Add `cargo clippy` in CI with `-D warnings` for `glass-*` crates only or
     configure third_party to be allowed to warn.

Medium-term (architectural)
- 6. Introduce `platform` abstraction in `glass-core` or a new `glass-platform`
     crate. Define a small trait surface for the OS features the framework uses:
     - `set_dpi_awareness()`
     - Window creation and styles for overlays
     - Tray icon, hotkey registration, message pump
     Implement a `platform-windows` crate that depends on `windows` and is the
     default implementation behind a `windows` feature. This keeps the core
     API cross-platform-friendly.
- 7. Introduce a `renderer::Backend` trait that abstracts the minimal surface
     / swapchain operations; keep the DX12 path as `backend-dx12` behind a
     feature flag. This enables future `backend-vulkan` or `backend-soft`.
- 8. Isolate anti-cheat into an optional workspace crate `glass-safety` with
     a documented feature `gaming`. The main library should expose hooks for
     runtime guards but not perform them by default.

Long-term (DX/maintenance)
- 9. Evaluate reducing or documenting the wgpu fork maintenance burden.
     Add automation (scripts) to rebase the forked `wgpu` subtree and run a
     mechanical test-suite to detect regressions.
- 10. Provide a Windows CI image preinstalled with the MSVC Build Tools or
     use `actions/cache` and artifact caching to make contributor setup easier.
- 11. Consider adding a reproducible builder (e.g., a Docker image for
     cross-compile verification) or a GH Action that sets up the same
     environment documented in `CLAUDE.md` so new contributors can build
     without interactive system configuration.

API & ergonomics recommendations (developer-facing)
---------------------------------------------------
- Make `OverlayModule` land in `glass-core` (or `glass-overlay` public module)
  with thorough Rustdoc examples and ensure trait methods are clearly
  documented for implementers.
- Use typed newtypes for config fields (e.g., `Hotkey`, `Anchor`, `Color`) to
  make configuration less error-prone and easier to document.
- Add a `ModuleBuilder` helper for common patterns (text widget, metric
  widget) so authors can create modules with less boilerplate.

Testing & CI recommendations
---------------------------
- Add integration tests that run `glass-starter` in a headless or minimal
  way on CI (smoke-start/shutdown), and unit tests that simulate module
  lifecycles.
- Run `cargo clippy` and `cargo fmt --all -- --check` in a CI job and fail
  on warnings only for `glass-*` crates (exclude `third_party/*`).
- Add an optional `nightly` CI job to catch future compatibility issues early.

Documentation & onboarding
-------------------------
- Expand `README.md` with a quick tutorial: "add a module in 5 steps".
- Publish a short "migration guide" for people who used the old PoC
  (if public users exist) explaining `glass-starter` and how to enable
  `gaming` features.
- Create a short video or GIFs showing the overlay in action (optional but
  high value for adoption).

Security & privacy
-----------------
- Keep anti-cheat detection strictly opt-in and document exactly which
  APIs are used (the project already does this in `safety.rs` — surface
  that doc in `SECURITY.md`).
- Add a short privacy statement if the framework ever ships telemetry or
  error reporting (for the PoC this is not present — stay explicit).

Operational tasks (housekeeping)
------------------------------
- Move all machine-local scripts (`link-wrapper.cmd`, `/usr/local/bin/cargo`)
  into a `contrib/` directory, and document how to install them. Avoid
  machine-local writes by default; prefer opt-in steps (`contrib/setup-wsl2.sh`).
- Add `gitignore` entries for typical IDE/workflow files and Windows build
  artefacts; ensure `C:\Users\...` local files are never committed.

Minimal example of an abstraction (sketch)
-----------------------------------------
```rust
// in glass-core (or glass-platform)
pub trait WindowBackend {
    type Handle;
    fn set_dpi_awareness() -> Result<(), GlassError>;
    fn create_overlay_window(
        hotkey_vk: u32,
        timeout_ms: u32,
    ) -> Result<Self::Handle, GlassError>;
    fn run_message_loop<F: FnMut()>(handle: &Self::Handle, mut tick: F) -> Result<(), GlassError>;
}
```
A `platform-windows` crate would implement this trait using `windows` APIs.

Next steps & priorities
----------------------
1. Add contrib/ scripts and move machine-local wrappers there. Document install steps.
2. Add `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, and `SECURITY.md`.
3. Implement small `platform-windows` abstraction and wire `glass-starter` to it.
4. Add example modules and integration tests in CI.
5. Triage and upstream the necessary fixes/PRs to `wgpu` (or document
   clearly why the fork is maintained).

Appendix: checklist for "library readiness"
-------------------------------------------
- [x] Clear README + LICENSE
- [x] Starter app (renamed: `glass-starter`)
- [x] No default anti-cheat; gaming opt-in feature
- [x] Hot-reload config + documented example config
- [ ] Platform abstraction (TODO)
- [ ] Renderer backend abstraction (TODO)
- [ ] Integration CI tests (TODO)
- [ ] CONTRIBUTING/SECURITY/CODE_OF_CONDUCT (TODO)

