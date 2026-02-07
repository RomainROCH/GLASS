# Project Memory

## ⚠️ Active Warnings

- **Python shim**: NEVER use `python` or `python3`. ALWAYS use `./python <script>`. See [`CLAUDE.md`](../../CLAUDE.md) for full policy + troubleshooting.
- **RON syntax**: RON 0.9 uses tuple syntax `(a, b)` for Rust arrays, not `[a, b]`.
- **wgpu subtree**: `third_party/wgpu/` is a git subtree from `wgpu-fork/v24`. Use `./python sync_wgpu.py` to manage it.
- **Origin was wrong**: Origin previously pointed to `RomainROCH/wgpu.git` instead of `RomainROCH/GLASS-UltimateOverlay.git`. Fixed 2026-02-07.
- **Dual-crate patch**: `wgpu-hal` alone causes type conflicts; must also patch `wgpu-types` + `naga` in `[patch.crates-io]`.

## Lessons Learned

- 2026-02-07: wgpu subtree branch `glass-patch-v24.0.4` contained GLASS code, not wgpu code. Use `v24` branch instead.
- 2026-02-07: When patching a workspace crate via `[patch.crates-io]`, ALL sibling crates that share types across the API boundary need patching too, or you get duplicate-type errors.
