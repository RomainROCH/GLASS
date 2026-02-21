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
`/usr/local/bin/cargo` is a wrapper script that calls the Windows `cargo.exe`. It works in both interactive and non-interactive shells (no need to source `~/.bashrc`).
```bash
cargo build --workspace                              # Build all crates (generic overlay)
cargo build -p glass-starter                         # Build the starter example only
cargo build -p glass-starter --features test_mode   # With watermark
cargo build -p glass-starter --features gaming      # With anti-cheat detection
```

## Architecture Quick Ref

- Rust workspace: `glass-core`, `glass-overlay`, `glass-starter`
- `glass-starter` is the example/entry-point for the generic overlay framework
- Anti-cheat detection is opt-in via `--features gaming` (not enabled by default)
- wgpu v24 via git subtree at `third_party/wgpu/` (see `sync_wgpu.py`)
- `[patch.crates-io]` overrides: `wgpu-hal`, `wgpu-types`, `naga`
- Windows-only: DirectComposition + DX12 backend
- Config: RON 0.9 (tuple syntax for arrays)

## Build Troubleshooting — After a Visual Studio Update

The linker is invoked via `C:\Users\RomainROCH\.cargo\link-wrapper.cmd`, configured in
`C:\Users\RomainROCH\.cargo\config.toml`. The wrapper uses `dir /b /ad /o-n` to
auto-select the **latest** MSVC version under:

```
C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\
```

Routine VS updates (new MSVC toolset version) require **no manual changes** — the wrapper
resolves the version dynamically on every build.

### What can still break

| Scenario | Symptom | Fix |
|---|---|---|
| VS BuildTools 2022 uninstalled or path moved | Linker not found / `link.exe` error | Reinstall VS BuildTools 2022 or restore the path |
| BuildTools year changes (e.g. 2022 → 2025) | `dir` finds no versions; linker not found | Update `MSVC_BASE` in `link-wrapper.cmd` to the new year |
| `MSVC\` directory is empty (partial install) | Same linker error | Run VS Installer → Repair, or install the "MSVC build tools" component |

### How to verify

```bash
cargo build --workspace   # uses the /usr/local/bin/cargo wrapper script
```

If the build succeeds, the wrapper resolved the toolchain correctly.
For manual inspection from PowerShell:

```powershell
dir "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\" -Directory | Sort-Object Name -Descending | Select-Object -First 1
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
