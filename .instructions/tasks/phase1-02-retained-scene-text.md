---
schema: task/v1
id: task-000435
title: "Implement retained scene graph + text rendering (glyphon)"
type: feature
status: not-started
priority: high
owner: "unassigned"
skills: ["design", "planning-feature", "quality-auditor"]
depends_on: []
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Goal
Add a retained scene graph and efficient text rendering path to `glass-overlay` so that the renderer can avoid unnecessary work each frame. Implement a `SceneNode` enum (Rect / Text / Image), a dirty-flag system that drives incremental redraws, and a glyphon-based text renderer that uses pre-allocated GPU and CPU buffers to avoid per-frame allocations in steady-state.

## Acceptance Criteria ✅
- A `SceneNode` enum is defined with variants: `Rect`, `Text`, and `Image` and an API to create/update/remove nodes.
- Scene nodes expose a `dirty` flag (or equivalent) that marks them for re-upload or re-render when changed.
- The renderer consumes the retained scene and only re-builds GPU-side geometry/resources for nodes that are dirty; unchanged nodes are not re-uploaded or re-processed.
- Text rendering uses `glyphon` (or a documented replacement) to rasterize and layout text; glyph metrics and atlas usage are cached and re-used across frames.
- Buffers (vertex/index/instance and any glyph staging) are pre-allocated and resized rarely (on demand when capacity is exceeded), avoiding per-frame heap allocations and transient Vec growth in steady-state.
- The hot path (steady-state frames with no scene changes) allocates zero heap memory per frame and produces no GPU buffer uploads.
- Unit/integration tests or small bench harnesses validate: scene node dirty-flag correctness, that unchanged frames do not allocate, and that text appears correctly and metrics are stable between frames.

## Context & Links 🔗
- Existing Phase 0 tasks and PoC: `.instructions/tasks/phase0-03-triangle-render.md`, `.instructions/tasks/phase0-02-wgpu-dcomp-spike.md`.
- Relevant code areas to update: `glass-overlay` renderer, scene representation, and text rendering module (create `text`/`glyphon` module as needed).
- Notes about `glyphon`: prefer glyphon crate for shaping/rasterization; if platform limitations exist, document fallback.

## Plan / Approach 🛠️
1. Design the `SceneNode` API (Rust enum `SceneNode { Rect(RectProps), Text(TextProps), Image(ImageProps) }`) and a `Scene` container that owns nodes and exposes mutation APIs.
2. Add a `dirty` boolean (or generation counter) to nodes and `Scene` to track which nodes need re-upload.
3. Implement a render-layer that iterates retained nodes and only rebuilds geometry for dirty nodes; track GPU resource handles per node.
4. Integrate `glyphon` for text layout/rasterization; cache glyph atlases/metrics and expose a lightweight `TextProps` that references text, font, size, color, and transform.
5. Implement pre-allocated buffer pools for vertex/index data and glyph staging; allow growth but avoid shrink-to-fit per-frame.
6. Add tests / a small bench harness to assert no heap allocations in steady-state frames (see Validation Notes for approaches).
7. Add documentation (README or inline module docs) explaining scene graph invariants and the no-allocation steady-state requirement.

## Validation Notes 🔍
- Functional: Render sample scenes (mixed Rect/Text/Image) and visually verify output matches expectations.
- Performance: Use a debug harness that runs 1k frames with no scene changes and assert that heap allocation counters (e.g., using the `heaptrack` equivalent or instrumenting `std::alloc`) are zero or within a tight bound per frame.
- Allocation checks: Add unit tests that run a simple frame loop and use a global allocator wrapper or `jemalloc`/`mimalloc` stats to assert no new allocations per frame.
- Buffer reuse: Verify vertex/index buffers' capacity remains stable across steady-state frames and only grows on demand when larger content is submitted.
- Glyph caching: Validate the glyph atlas is reused across frames for identical text/font/size combinations and that glyph uploads happen only on the first use.

## Notes / Questions ❓
- Preferred rust crate for glyph/text shaping: confirm `glyphon` is acceptable (or list an allowed fallback).
- Desired API ergonomics for the `Scene` (ownership model, thread-safety expectations). For now implement a single-threaded, immediate-mutating API unless the team requests an immutable or multi-threaded design.

## Next Steps ➡️
- Assign an owner and prioritize integration with `glass-overlay` crate.
- Create follow-up tasks for: (a) creating a bench harness and allocation assertions, (b) extending CI to run the allocation checks on supported environments.
