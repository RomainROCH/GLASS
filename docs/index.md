---
created: 2026-03-26
updated: 2026-03-26
category: user
status: active
doc_kind: index
---

# GLASS docs

Use this folder as the fast path into the project.

## Start with one of these

- **I want to run GLASS now** → [`getting-started.md`](getting-started.md)
- **I want to build my own overlay app** → [`library-consumer.md`](library-consumer.md)
- **I want to add my own module/widget** → [`module-authoring.md`](module-authoring.md)

## Recommended reading order

1. [`../README.md`](../README.md) for the high-level architecture and feature set
2. [`getting-started.md`](getting-started.md) for the quickest first run
3. [`library-consumer.md`](library-consumer.md) if you are integrating `glass-overlay`
4. [`module-authoring.md`](module-authoring.md) if you are extending the overlay

## Canonical code entry points

- [`../glass-starter/examples/minimal.rs`](../glass-starter/examples/minimal.rs) — smallest end-to-end bootstrap
- [`../glass-starter/src/main.rs`](../glass-starter/src/main.rs) — full reference app wiring
- [`../glass-overlay/src/lib.rs`](../glass-overlay/src/lib.rs) — crate-root API and re-exports
