---
schema: task/v1
id: task-000001
title: "Scaffold Rust workspace: Initialize Cargo workspace & DX12-only config"
type: chore
status: archived
priority: medium
owner: "executive2"
skills: ["planning-feature", "docs", "system-editor"]
depends_on: []
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Goal
Initialize a Rust Cargo workspace for the project with three crates: `glass-core`, `glass-overlay`, and `glass-poc`. Add DX12-only developer-experience configuration and basic repository tooling (rust-toolchain, `.cargo/config.toml`, `.gitignore`, `rustfmt` and `clippy` configs). Do not implement product code beyond minimal scaffolding and manifests.

## Acceptance Criteria ✅
- A workspace root `Cargo.toml` exists and lists `glass-core`, `glass-overlay`, and `glass-poc` as members.
- A `rust-toolchain` file is present and pins a stable toolchain (team to confirm exact version).
- A `.cargo/config.toml` exists with a DX12-only configuration comment and any necessary target or wgpu backend hints (DX12 only).
- Each member crate has a minimal `Cargo.toml` (name, edition) and an empty `src/lib.rs` or `src/main.rs` to make it a valid crate (no product logic beyond scaffolding).
- Workspace or crate-level `Cargo.toml` includes dependency entries or explicit placeholders for:
  - `wgpu` (version >= 23) — marked **DX12-only**
  - `windows` / `windows-sys` (windows-rs) — note intended usage
  - `tracing` and `tracing-tracy` (or `tracing` + `tracing-tracy` integration)
  - `raw-window-handle`
  These may be listed as comments / explicit version placeholders if team prefers adding exact versions later.
- Repository tooling files added: `.gitignore` with Rust defaults, `rustfmt.toml`, and a `clippy` configuration (e.g., `clippy.toml` or add clippy settings to `Cargo.toml`/CI notes).
- `.instructions/architecture.md` is updated (via a follow-up change) to include a brief Architecture Spec Summary for the Rust workspace and intended responsibilities of `glass-core`, `glass-overlay`, and `glass-poc`.
- No product feature code is added — only scaffolding, manifests, and configuration files.

## Context & Links 🔗
- Existing repo instructions: `.instructions/architecture.md` (currently placeholder; this task should add a short architecture spec summary as part of the follow-up work).
- Suggest creating a plan artefact for Phase 0 scaffolding if needed: `.instructions/artefacts/phase0-scaffolding-plan.md` (none found during intake).

## Plan / Approach 🛠️
1. Create a workspace `Cargo.toml` with the three members.
2. Add `rust-toolchain` file (pinning to `stable` or a specific date/version after confirmation).
3. Add `.cargo/config.toml` with DX12-specific configuration notes (e.g., preferred backend setting to DX12, any feature flags for `wgpu`).
4. Create each crate folder (`glass-core`, `glass-overlay`, `glass-poc`) with a minimal `Cargo.toml` and `src/` skeleton.
5. Add placeholder dependency entries in workspace `Cargo.toml` or crate `Cargo.toml` files for `wgpu >= 23`, `windows` (windows-rs), `tracing`/`tracing-tracy`, and `raw-window-handle` and annotate DX12-only items clearly.
6. Add repository tooling files: `.gitignore`, `rustfmt.toml`, and clippy config notes / file.
7. Draft the short Architecture Spec Summary and add it to `.instructions/architecture.md` as a separate follow-up commit (this task includes the work item to update that file; actual edit can be done in the next task or by the assignee).

## Validation Notes 🔍
- Run `cargo metadata` in the workspace root to confirm members are discovered.
- Verify `rustup show` or `rustup override list` respects the `rust-toolchain` file.
- Run `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings` (or equivalent CI steps) to validate formatting and lint config are present.
- Confirm `.cargo/config.toml` contains explicit DX12-note and any target/backend hints; verify no other backends are enabled by default.
- Ensure `.instructions/architecture.md` is updated or that a follow-up task is created to perform the update with a brief spec summary.

## Notes / Questions ❓
- Confirm the desired toolchain pin (e.g., `stable`, `1.71.0`, or a date), and whether a `rust-toolchain.toml` with components (clippy, rustfmt) is preferred.
- Confirm if `wgpu` should be added to the workspace `[dependencies]` or only per-crate that needs it.

## Next Steps ➡️
- Assign an owner to start the scaffolding work.
- Confirm toolchain pin and whether the team wants the scaffolding to include minimal CI changes (optional).
- Optionally: create a follow-up task to actually update `.instructions/architecture.md` with the Architecture Spec Summary (this task includes that as a required follow-up step).
