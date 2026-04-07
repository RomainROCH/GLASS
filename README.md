# GLASS

GLASS is a Windows overlay framework for Rust. It creates a transparent DirectComposition-backed overlay window, renders through `wgpu` on DX12, keeps overlay content in a retained scene graph, and organizes on-screen widgets as modular `OverlayModule` implementations. The workspace includes the reusable library crates and a reference starter app that shows the intended integration flow.

## Status

GLASS is pre-1.0 software. The API is usable, but it may still change as the library is hardened for external consumers.

## Key features

- transparent Windows overlay using DirectComposition
- `wgpu` renderer on DX12
- retained scene graph with explicit scene invalidation
- anchor-based module layout
- modular widget system via `OverlayModule`
- config loading and file watching through `ConfigStore`
- passive and interactive input modes
- reference app in `glass-starter`

## Quick start

Requirements:

- Windows 10 or Windows 11
- Rust stable 1.85+
- MSVC build tools

```sh
git clone https://github.com/RomainROCH/GLASS
cd GLASS
cargo build --workspace
cargo run -p glass-starter
```

For the smallest current bootstrap:

```sh
cargo run --example minimal -p glass-starter
```

## Documentation

- [`docs/index.md`](docs/index.md) — docs hub
- [`docs/getting-started.md`](docs/getting-started.md) — first run
- [`docs/library-consumer.md`](docs/library-consumer.md) — build your own app with GLASS
- [`docs/module-authoring.md`](docs/module-authoring.md) — write a custom module

## Architecture

The workspace is split into three main crates:

- `glass-core` — shared core types, including `GlassError`
- `glass-overlay` — the reusable overlay library: config, layout, scene graph, modules, windowing, compositor, renderer
- `glass-starter` — the reference application and smallest runnable examples

If you want to study the current reference flow, start with:

- [`glass-starter/src/main.rs`](glass-starter/src/main.rs)
- [`glass-starter/examples/minimal.rs`](glass-starter/examples/minimal.rs)

## Platform support

GLASS currently supports Windows 10 and Windows 11 only. The runtime depends on Win32 APIs, DirectComposition, and a DX12-backed `wgpu` surface path, so it is not a cross-platform overlay framework today.

## Notes on current behavior

- `SystemStatsModule` does not perform built-in temperature detection. Applications inject a callback with `SystemStatsModule::set_temp_source()`.
- `ConfigStore::watch()` updates the stored config snapshot, but a running app must still re-read and reapply the snapshot for runtime behavior to change. The reference starter does not currently do that after startup.

## License

Dual licensed under Apache-2.0 or MIT. See [`LICENSE-APACHE`](LICENSE-APACHE) and [`LICENSE-MIT`](LICENSE-MIT).
