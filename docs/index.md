---
created: 2026-03-26
updated: 2026-04-07
category: user
status: active
doc_kind: index
---

# GLASS docs

Use this folder when you want the shortest path from clone to a working GLASS integration.

## Documentation map

| Document | Covers | Best for |
|---|---|---|
| [`getting-started.md`](getting-started.md) | Clone, build, and run the reference app | First-time users |
| [`library-consumer.md`](library-consumer.md) | Using `glass-overlay` in your own app | Library consumers |
| [`module-authoring.md`](module-authoring.md) | Writing a custom `OverlayModule` | Module/widget authors |
| [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.md) | System design, layers, decisions, ecosystem vision | Architects, LLM developers, contributors |
| [`../README.md`](../README.md) | Project overview, architecture, platform support, links | Repository overview |

## Recommended reading order

1. [`../README.md`](../README.md)
2. [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.md)
3. [`getting-started.md`](getting-started.md)
4. [`library-consumer.md`](library-consumer.md)
5. [`module-authoring.md`](module-authoring.md)

## Canonical code entry points

- [`../glass-starter/src/main.rs`](../glass-starter/src/main.rs) — full reference flow
- [`../glass-starter/examples/minimal.rs`](../glass-starter/examples/minimal.rs) — smallest current bootstrap
- [`../glass-overlay/src/lib.rs`](../glass-overlay/src/lib.rs) — crate-root consumer API

## What to read next

- Want to run GLASS now? Start with [`getting-started.md`](getting-started.md).
- Want to embed GLASS in your own binary? Go to [`library-consumer.md`](library-consumer.md).
- Want to build a custom widget/module? Go to [`module-authoring.md`](module-authoring.md).

## Architecture deep-dives

| Document | Covers |
|---|---|
| [`architecture/ARCHITECTURE.md`](architecture/ARCHITECTURE.md) | Architecture overview — philosophy, layers, ecosystem vision |
| [`architecture/decisions.md`](architecture/decisions.md) | Architecture Decision Records (ADR-001 through ADR-007) |
| [`architecture/composition-pipeline.md`](architecture/composition-pipeline.md) | DirectComposition + wgpu binding, alpha patches, HDR |
| [`architecture/scene-graph.md`](architecture/scene-graph.md) | Retained scene graph, dirty tracking, zero-alloc steady state |
| [`architecture/module-system.md`](architecture/module-system.md) | OverlayModule trait, layout, callback injection |
| [`architecture/input-system.md`](architecture/input-system.md) | Passive/interactive modes, hit-testing, hotkeys |
| [`architecture/config-system.md`](architecture/config-system.md) | RON/TOML loading, ArcSwap hot-reload |
| [`architecture/safety-system.md`](architecture/safety-system.md) | Anti-cheat detection, feature-gated safety |
