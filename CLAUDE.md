# GLASS Project - AI Assistant Rules

## [CRITICAL] Python Execution Policy

**NEVER use global `python` command.**

**ALWAYS use:**
```bash
./python sync_wgpu.py <command>
```

**WHY:** This project uses `uv` for isolated Python environment management.
The `./python` shim ensures zero pollution of the Windows system and automatic
dependency management.

If you see `command not found: python` — that is expected.
The fix is **always** `./python` (not `python`, not `python3`, not `py`).

## Build
Utiliser le cargo Windows natif (alias configuré dans .bashrc) :
```bash
cargo build --workspace
```

## Available Commands

### Direct execution
- `./python sync_wgpu.py status` - Show subtree status
- `./python sync_wgpu.py pull` - Pull changes from wgpu fork
- `./python sync_wgpu.py push` - Push changes to wgpu fork
- `./python sync_wgpu.py setup` - Initial subtree + remote setup

### Via task runner (shortcuts)
- `./tasks.sh sync-status` - Show subtree status
- `./tasks.sh sync-pull` - Pull changes from wgpu fork
- `./tasks.sh sync-push` - Push changes to wgpu fork
- `./tasks.sh sync-init` - Resync Python dependencies

## Adding Python Dependencies
```bash
uv add <package-name>
```

**Never use:** `pip install` (breaks isolation)

## Environment

- Python: Managed by uv (currently 3.14.3)
- Virtual env: `.venv/` (auto-created, gitignored)
- Dependencies: Defined in `pyproject.toml`

## Troubleshooting

| Error | Cause | Fix |
|---|---|---|
| `bash: python: command not found` | Bare `python` not on PATH (by design) | Use `./python` instead |
| `bash: python3: command not found` | Same as above | Use `./python` instead |
| `uv: command not found` | uv not installed or PATH not loaded | Install uv: `curl -LsSf https://astral.sh/uv/install.sh \| sh` |
| `Working tree is dirty` (sync_wgpu.py) | Uncommitted changes block subtree ops | `git add . && git commit` first |
| `error inheriting ... from workspace` | wgpu-hal resolved wrong workspace root | Ensure `exclude = ["third_party/wgpu"]` in root `Cargo.toml` |
| `multiple versions of crate wgpu_types` | Missing `[patch.crates-io]` entry | Patch `wgpu-hal`, `wgpu-types`, AND `naga` together |
