---
created: 2026-03-26
updated: 2026-03-26
category: user
status: active
doc_kind: guide
---

# Getting started

This page is the quickest way to get from clone to a running GLASS overlay.

## Prerequisites

- Windows 10 1903+ with DirectComposition and DX12 support
- Rust stable 1.85+
- MSVC Build Tools

## Run the reference starter

```sh
cargo run -p glass-starter
```

Use this when you want the full reference app: config loading, built-in modules, layout, input handling, and the normal message loop.

## Run the minimal example

```sh
cargo run --example minimal -p glass-starter
```

Use this when you want the smallest possible GLASS integration. This example is the best first code file to inspect if you are embedding GLASS into another app.

## What happens on first run

When you run `glass-starter`:

1. tracing/logging is initialized
2. the process is marked DPI-aware
3. `config.ron` is loaded from the current working directory
4. if `config.ron` does not exist yet, it is created with defaults
5. the config watcher starts so updated config snapshots are reloaded into `ConfigStore`
6. the overlay window, DirectComposition compositor, renderer, layout manager, and built-in widgets are initialized

When you run the `minimal` example, there is no config or widget setup. It just boots the overlay runtime with the smallest amount of code.

The current starter consumes the config fields it actually wires: input hotkey/timeout behavior, modules, and layout. `position`, `size`, `opacity`, and `input.show_indicator` are currently stored schema values only and are not applied by the reference starter runtime, including after restart.

## Where config lives

- **Reference starter:** `config.ron` in the working directory by default
- **Library consumers:** you choose the path and format by calling `ConfigStore::load(...)`

Examples:

```rust
use glass_overlay::ConfigStore;

let ron = ConfigStore::load("config.ron")?;
let toml = ConfigStore::load("config.toml")?;
```

The format is selected from the file extension.

## Common first things to inspect

- [`../glass-starter/examples/minimal.rs`](../glass-starter/examples/minimal.rs) — smallest integration
- [`../glass-starter/src/main.rs`](../glass-starter/src/main.rs) — full reference bootstrap
- [`../glass-overlay/src/lib.rs`](../glass-overlay/src/lib.rs) — recommended crate-root imports
- [`../config.ron`](../config.ron) — starter config shape and defaults

## Next docs

- Building your own app → [`library-consumer.md`](library-consumer.md)
- Adding your own module → [`module-authoring.md`](module-authoring.md)
- Repository overview → [`../README.md`](../README.md)
