# Retained Scene Graph

This document describes the retained scene graph that sits at the core of GLASS
rendering. All visual content — text, rectangles, and future node types — is
represented as nodes in this graph. The renderer only does GPU work when nodes
change.

**Source file:** `scene.rs`

**See also:** [ARCHITECTURE.md](ARCHITECTURE.md) ·
[composition-pipeline.md](composition-pipeline.md)

---

## Table of Contents

1. [Design Philosophy](#design-philosophy)
2. [Core Types](#core-types)
3. [Scene API](#scene-api)
4. [Dirty Tracking](#dirty-tracking)
5. [Node Lifecycle](#node-lifecycle)
6. [Interaction with Modules](#interaction-with-modules)
7. [Thread Safety](#thread-safety)
8. [Future Extension Points](#future-extension-points)

---

## Design Philosophy

GLASS uses a **retained** scene graph, not an immediate-mode UI.

Most overlay frames are identical. Data updates arrive every 1–2 seconds (clock
ticks, system stats refresh), but the overlay renders at display refresh rate.
In an immediate-mode model, every frame would rebuild and re-upload all draw
data — wasting CPU and GPU cycles for frames that are visually identical.

The retained model with dirty tracking eliminates this waste:

| State | GPU work | Heap allocations |
|---|---|---|
| Steady state (no mutations) | **Zero** — renderer skips work | **Zero** — no buffers touched |
| Mutation frame (node add/update/remove) | Re-upload changed data | Minimal (only changed nodes) |

The key invariant: **if `scene.is_dirty() == false`, the renderer has nothing
to do.** The entire prepare → acquire → encode → submit → present pipeline can
be short-circuited.

---

## Core Types

### `NodeId`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);
```

Unique identifier for a scene node. IDs are monotonically increasing — a
removed ID is never reused. This makes it safe for modules to hold `NodeId`
values as stable handles across frames.

`NodeId` implements `Display` as `NodeId(N)` for diagnostic logging.

### `Color`

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,  // [0.0, 1.0]
    pub g: f32,  // [0.0, 1.0]
    pub b: f32,  // [0.0, 1.0]
    pub a: f32,  // [0.0, 1.0]
}
```

RGBA color in **premultiplied alpha** space. For premultiplied colors, RGB
values should already be multiplied by alpha before storage.

| Method / Constant | Description |
|---|---|
| `Color::new(r, g, b, a)` | Construct from components (const fn) |
| `color.premultiply()` | Returns a new `Color` with `r*a, g*a, b*a, a` |
| `Color::TRANSPARENT` | `(0.0, 0.0, 0.0, 0.0)` — fully transparent |
| `Color::WHITE` | `(1.0, 1.0, 1.0, 1.0)` — opaque white |
| `Color::BLACK` | `(0.0, 0.0, 0.0, 1.0)` — opaque black |

### `RectProps`

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RectProps {
    pub x: f32,       // top-left X in screen pixels
    pub y: f32,       // top-left Y in screen pixels
    pub width: f32,   // width in pixels
    pub height: f32,  // height in pixels
    pub color: Color, // fill color (premultiplied alpha)
}
```

Properties for a solid-color rectangle node.

### `TextProps`

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TextProps {
    pub x: f32,           // baseline origin X in screen pixels
    pub y: f32,           // baseline origin Y in screen pixels
    pub text: String,     // text content
    pub font_size: f32,   // font size in logical pixels
    pub color: Color,     // text color (premultiplied alpha)
}
```

Properties for a text node rendered via glyphon. The `text_renderer` module
collects all `Text` nodes each frame, lays them out with cosmic-text, and
renders them in a single GPU pass.

### `SceneNode`

```rust
#[derive(Debug, Clone)]
pub enum SceneNode {
    Rect(RectProps),
    Text(TextProps),
}
```

The public node type. Modules create and update nodes using this enum.

### `NodeEntry` (internal)

```rust
struct NodeEntry {
    id: NodeId,
    node: SceneNode,
    dirty: bool,
    generation: u64,
}
```

Internal storage type. Each entry wraps a `SceneNode` with its `NodeId`, a
per-node `dirty` flag, and a `generation` counter that increments on every
update. The generation counter enables future incremental-upload optimizations.

---

## Scene API

### `Scene` struct

```rust
pub struct Scene {
    nodes: Vec<NodeEntry>,
    next_id: u32,
    dirty: bool,  // global dirty flag
}
```

The scene is a flat `Vec` of node entries. There is no tree structure (yet) —
all nodes are siblings at the same level, rendered in insertion order.

### Methods

| Method | Signature | Returns | Effect |
|---|---|---|---|
| `new()` | `fn new() -> Self` | Empty scene | Not dirty |
| `add_rect(props)` | `fn add_rect(&mut self, props: RectProps) -> NodeId` | New `NodeId` | Adds node, marks dirty |
| `add_text(props)` | `fn add_text(&mut self, props: TextProps) -> NodeId` | New `NodeId` | Adds node, marks dirty |
| `update(id, node)` | `fn update(&mut self, id: NodeId, node: SceneNode) -> bool` | `true` if found | Updates node, marks dirty, increments generation |
| `remove(id)` | `fn remove(&mut self, id: NodeId) -> bool` | `true` if found | Removes node, marks dirty |
| `is_dirty()` | `fn is_dirty(&self) -> bool` | `bool` | Global dirty flag |
| `iter()` | `fn iter(&self) -> impl Iterator<Item = (NodeId, &SceneNode)>` | Iterator | All nodes (read-only) |
| `dirty_nodes()` | `fn dirty_nodes(&self) -> impl Iterator<Item = (NodeId, &SceneNode)>` | Iterator | Only dirty nodes |
| `clear_dirty()` | `fn clear_dirty(&mut self)` | — | Resets all dirty flags (per-node + global) |
| `len()` | `fn len(&self) -> usize` | Count | Number of nodes |
| `is_empty()` | `fn is_empty(&self) -> bool` | `bool` | Whether scene has zero nodes |

### Usage example

```rust
use glass_overlay::{Scene, TextProps, Color, NodeId, SceneNode};

let mut scene = Scene::new();

// Add a text node
let id = scene.add_text(TextProps {
    x: 10.0,
    y: 10.0,
    text: "CPU: 42%".into(),
    font_size: 16.0,
    color: Color::WHITE,
});

assert!(scene.is_dirty());

// After render:
scene.clear_dirty();
assert!(!scene.is_dirty());

// Update the text later:
scene.update(id, SceneNode::Text(TextProps {
    x: 10.0,
    y: 10.0,
    text: "CPU: 55%".into(),
    font_size: 16.0,
    color: Color::WHITE,
}));

assert!(scene.is_dirty());
```

---

## Dirty Tracking

Dirty tracking operates at two levels:

```
Scene::dirty          (global)   ← any mutation sets this
  └─ NodeEntry::dirty (per-node) ← only the mutated node(s)
```

### State transitions

| Operation | `Scene::dirty` | Affected `NodeEntry::dirty` |
|---|---|---|
| `add_rect()` / `add_text()` | → `true` | New entry: `true` |
| `update(id, node)` | → `true` | Target entry: `true`, generation `+= 1` |
| `remove(id)` | → `true` | Entry removed from vec |
| `clear_dirty()` | → `false` | All entries: `false` |
| No mutation | Stays `false` | All stay `false` |

### Render integration

The render loop checks `scene.is_dirty()` to decide whether GPU work is
needed. After a successful render, `scene.clear_dirty()` resets everything.
In steady state — when modules are not updating — the dirty flag stays `false`
indefinitely.

```
Message loop tick:
  modules.update_all(scene, dt)    // may mutate nodes
  if scene.is_dirty() {
      renderer.render()            // GPU work
      scene.clear_dirty()          // reset flags
  }
```

The `dirty_nodes()` iterator exists for future incremental re-upload. Currently,
the text engine re-prepares all text nodes each dirty frame. As the node count
grows, switching to incremental upload using `dirty_nodes()` will reduce
per-frame work.

---

## Node Lifecycle

```
Module::init()                    Module::update()
     │                                 │
     ▼                                 ▼
 ┌────────────┐    update(id, ..)  ┌────────────┐
 │  add_text() │ ─────────────────▶│  scene     │
 │  add_rect() │                   │  .update() │
 └─────┬──────┘                    └─────┬──────┘
       │                                 │
       │ NodeId returned                 │ dirty = true
       │                                 │ generation += 1
       ▼                                 ▼
 ┌──────────────────────────────────────────────┐
 │              Scene (Vec<NodeEntry>)           │
 │                                              │
 │  [ NodeEntry { id: 0, dirty: T, gen: 0 } ]  │
 │  [ NodeEntry { id: 1, dirty: F, gen: 3 } ]  │
 │  [ NodeEntry { id: 2, dirty: T, gen: 0 } ]  │
 │                                              │
 │  Global dirty: true                          │
 └──────────────────────┬───────────────────────┘
                        │
                        │ render() + clear_dirty()
                        ▼
 ┌──────────────────────────────────────────────┐
 │  All dirty flags → false                     │
 │  Global dirty → false                        │
 │  Steady state: zero GPU work per frame       │
 └──────────────────────────────────────────────┘
                        │
                        │ Module::deinit()
                        ▼
 ┌──────────────────────────────────────────────┐
 │  remove(id) for each owned NodeId            │
 │  Entries removed from vec                    │
 │  Global dirty → true (triggers re-render)    │
 └──────────────────────────────────────────────┘
```

---

## Interaction with Modules

Modules interact with the scene graph through the `OverlayModule` trait
lifecycle:

```rust
pub trait OverlayModule {
    fn init(&mut self, scene: &mut Scene);
    fn update(&mut self, scene: &mut Scene, dt: Duration) -> bool;
    fn deinit(&mut self, scene: &mut Scene);
    // ...
}
```

### Ownership contract

Each module **owns its `NodeId` values**. The typical pattern:

```rust
struct MyModule {
    node_ids: Vec<NodeId>,
    // ...
}

impl OverlayModule for MyModule {
    fn init(&mut self, scene: &mut Scene) {
        let id = scene.add_text(TextProps { /* ... */ });
        self.node_ids.push(id);
    }

    fn update(&mut self, scene: &mut Scene, dt: Duration) -> bool {
        // Update existing nodes by ID
        scene.update(self.node_ids[0], SceneNode::Text(TextProps { /* ... */ }))
    }

    fn deinit(&mut self, scene: &mut Scene) {
        // Clean removal using the helper
        remove_nodes(scene, &mut self.node_ids);
    }
}
```

### `remove_nodes()` helper

```rust
pub(crate) fn remove_nodes(scene: &mut Scene, ids: &mut Vec<NodeId>) {
    for id in ids.drain(..) {
        scene.remove(id);
    }
}
```

Drains the module's ID vector and removes each node from the scene. After this
call, `ids` is empty and the scene is marked dirty.

---

## Thread Safety

The scene graph is **single-threaded**. It has an immediate-mutating API with no
interior mutability, no locks, and no atomic operations.

All scene access happens on the **message-loop thread**:
- Module `init()` / `update()` / `deinit()` calls mutate the scene.
- `Renderer::render()` reads the scene and clears dirty flags.
- Both run synchronously within the Win32 `GetMessage` / `DispatchMessage` loop.

`Scene` is neither `Send` nor `Sync`. Attempting to share it across threads is
a compile-time error.

---

## Future Extension Points

### Group node type

A `Group` node type for ordered children is planned. This would enable
hierarchical transforms, clipping, and batch visibility toggling:

```rust
// Future:
pub enum SceneNode {
    Rect(RectProps),
    Text(TextProps),
    Group { children: Vec<NodeId> },
}
```

### Custom node types

The `SceneNode` enum can be extended with new variants without changing the
scene graph architecture:

- **`Image(ImageProps)`** — texture-backed sprites for icons or graphs
- **`CustomPaint(PaintFn)`** — arbitrary wgpu render commands
- **`Path(PathProps)`** — vector paths for charts or indicators

Each new type requires a corresponding rendering backend (analogous to how
`Text` nodes are handled by `TextEngine`).

### Scaling characteristics

The flat `Vec<NodeEntry>` structure scales comfortably to **hundreds of nodes**.
The performance bottleneck at scale is GPU text rendering, not scene traversal.
glyphon's atlas-based approach handles large text node counts well. If scene
traversal becomes a bottleneck (thousands of nodes), the flat vec can be
replaced with a slotmap for O(1) lookup by ID.

---

## Related Documents

- [ARCHITECTURE.md](ARCHITECTURE.md) — high-level system overview
- [composition-pipeline.md](composition-pipeline.md) — rendering pipeline, wgpu setup, text engine
- [decisions.md](decisions.md) — ADR log including retained-vs-immediate rationale
