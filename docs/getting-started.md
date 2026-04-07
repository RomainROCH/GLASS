---
created: 2026-03-26
updated: 2026-04-07
category: user
status: active
doc_kind: guide
---

# Getting started

This is the fastest path from `git clone` to a running GLASS overlay.

## Requirements

- Windows 10 or Windows 11
- A system with DirectComposition and DX12 support
- Rust stable 1.85+
- MSVC build tools

GLASS is Windows-only because it depends on Win32 windowing, DirectComposition, and a DX12 `wgpu` backend.

## Quick start

```sh
git clone https://github.com/RomainROCH/GLASS
cd GLASS
cargo build --workspace
cargo run -p glass-starter
```

## What you should see

On a default run, `glass-starter` creates a transparent fullscreen overlay and renders the built-in modules from `config.ron`:

- a clock
- system CPU/RAM stats
- an FPS counter

The starter also demonstrates the current temperature injection pattern. Its placeholder callback returns no value, so the system stats module shows `temp: N/A` until your own app provides a real temperature source.

The overlay starts in passive click-through mode. A hotkey can switch it into interactive mode using the values loaded from `config.ron`.

## First-run behavior

When `glass-starter` starts, it:

1. enables DPI awareness
2. loads `config.ron`
3. creates `config.ron` with defaults if it does not exist yet
4. starts a config watcher
5. creates the overlay window, DirectComposition compositor, renderer, layout manager, and built-in modules
6. renders once and enters the message loop

`ConfigStore::watch()` refreshes the stored config snapshot, but the running app must still re-read and reapply that snapshot for behavior to change. The reference starter does not currently reapply watched config after startup.

## Smallest example

If you want the minimum possible integration, run:

```sh
cargo run --example minimal -p glass-starter
```

That example only creates the window, compositor, renderer, minimal input/layout state, renders once, and enters the message loop.

## Next steps

- Build your own app with the library: [`library-consumer.md`](library-consumer.md)
- Write a custom module: [`module-authoring.md`](module-authoring.md)
- Review the project overview: [`../README.md`](../README.md)
