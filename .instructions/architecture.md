# Architecture

## Overview

- **Rust workspace** (`glass-core`, `glass-overlay`, `glass-poc`) targeting Windows x86_64.
- **wgpu v24** via git subtree at `third_party/wgpu/` — managed by `sync_wgpu.py`.
- **DirectComposition** pipeline for per-pixel-alpha transparent overlay on DX12.
- `[patch.crates-io]` overrides `wgpu-hal`, `wgpu-types`, `naga` from the subtree.
- Workspace `exclude = ["third_party/wgpu"]` prevents cargo workspace conflict.

## Patterns & Conventions

- Retained rendering with dirty-flag scene graph (zero per-frame allocations).
- Conditional compilation via Cargo features (`test_mode`, `alloc-tracking`).
- Config serialization: RON 0.9 (uses tuple syntax `()` for arrays, not `[]`).
- Git branching: conventional commits, feature branches merged into `master`.

## [CRITICAL] Python Execution

**NEVER use bare `python` / `python3`.** Always use `./python <script>`.  
See [`CLAUDE.md`](../../CLAUDE.md) for full policy + troubleshooting.
