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
| [`../README.md`](../README.md) | Project overview, architecture, platform support, links | Repository overview |

## Recommended reading order

1. [`../README.md`](../README.md)
2. [`getting-started.md`](getting-started.md)
3. [`library-consumer.md`](library-consumer.md)
4. [`module-authoring.md`](module-authoring.md)

## Canonical code entry points

- [`../glass-starter/src/main.rs`](../glass-starter/src/main.rs) — full reference flow
- [`../glass-starter/examples/minimal.rs`](../glass-starter/examples/minimal.rs) — smallest current bootstrap
- [`../glass-overlay/src/lib.rs`](../glass-overlay/src/lib.rs) — crate-root consumer API

## What to read next

- Want to run GLASS now? Start with [`getting-started.md`](getting-started.md).
- Want to embed GLASS in your own binary? Go to [`library-consumer.md`](library-consumer.md).
- Want to build a custom widget/module? Go to [`module-authoring.md`](module-authoring.md).
