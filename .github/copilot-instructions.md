# GLASS Project — Copilot Instructions

## [CRITICAL] Python Execution Policy

**NEVER use bare `python` or `python3` commands.** They will fail (not on PATH).

**ALWAYS prefix with the project shim:**
```bash
./python sync_wgpu.py <command>
```

This project uses `uv` for isolated Python environment management.
The `./python` shim ensures the correct venv, zero system pollution, and automatic dependency resolution.

If you see `command not found: python` — that is expected.  
The fix is **always** `./python` (not `python`, not `python3`, not `py`).

Full Python policy, troubleshooting table, and available commands: see [`CLAUDE.md`](../CLAUDE.md).

## Available Python Commands

- `./python sync_wgpu.py status` — Show git-subtree status for wgpu
- `./python sync_wgpu.py pull`   — Pull upstream wgpu changes (squash)
- `./python sync_wgpu.py push`   — Push local wgpu-hal patches to fork
- `./python sync_wgpu.py setup`  — Initial subtree + remote setup

## Build

```bash
cargo build --workspace          # Build all crates
cargo build -p glass-starter         # Build the starter harness only
cargo build -p glass-starter --features test_mode  # With watermark
```

## Architecture Quick Ref

- Rust workspace: `glass-core`, `glass-overlay`, `glass-starter`
- wgpu v24 via git subtree at `third_party/wgpu/` (see `sync_wgpu.py`)
- `[patch.crates-io]` overrides: `wgpu-hal`, `wgpu-types`, `naga`
- Windows-only: DirectComposition + DX12 backend
- Config: RON 0.9 (tuple syntax for arrays)
