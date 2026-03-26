---
created: 2026-03-26
updated: 2026-03-26
category: user
status: active
doc_kind: guide
---

# Module authoring

GLASS modules are small units of overlay behavior that own their own scene nodes and plug into layout through `WidgetWrapper`.

## `OverlayModule`

Implement `OverlayModule` for any custom widget/module you want to place on the overlay.

Recommended imports from the crate root:

```rust
use glass_overlay::{
    Color, ModuleInfo, NodeId, OverlayModule, Scene, TextProps,
};
use std::time::Duration;
```

The important responsibilities are:

- `info()` — stable metadata for the module
- `init(scene)` — create initial scene nodes
- `update(scene, dt)` — update content; return `true` when the scene changed
- `deinit(scene)` — remove any nodes you created
- `set_position(x, y)` — receive coordinates from layout
- `content_size()` — provide an estimated size for layout and hit-testing

## Key scene and layout imports

Most custom modules need a small set of types from the crate root:

```rust
use glass_overlay::{
    Anchor, Color, LayoutManager, ModuleInfo, NodeId, OverlayModule, Scene, TextProps,
    WidgetWrapper,
};
```

This keeps consumer code on the intended top-level API instead of reaching into internal module paths.

## Lifecycle expectations

| Method | When it runs | What to do |
|--------|--------------|------------|
| `init(scene)` | When the module becomes active | Add scene nodes |
| `update(scene, dt)` | On each tick while enabled | Refresh data, mutate nodes if needed |
| `deinit(scene)` | On disable or shutdown | Remove all nodes created by the module |
| `set_position(x, y)` | When layout computes a new anchor position | Store coordinates for future draws/updates |
| `content_size()` | During layout and hit-testing | Return a conservative `(width, height)` |

Practical rule: if your module created a node in `init`, it should clean it up in `deinit`.

## Registering a module with `WidgetWrapper`

`WidgetWrapper` handles the screen-relative placement. Your module handles content.

```rust
use glass_overlay::{Anchor, LayoutManager, WidgetWrapper};

let mut layout_manager = LayoutManager::new(screen_w, screen_h);

layout_manager.add_widget(WidgetWrapper::new(
    MyModule::new(),
    Anchor::TopRight,
    20.0,
    20.0,
));
```

After registration, initialize widgets/modules against the scene:

```rust
layout_manager.init_all(renderer.scene_mut());
```

## Minimal module sketch

```rust
use glass_overlay::{
    Color, ModuleInfo, NodeId, OverlayModule, Scene, TextProps,
};
use std::time::Duration;

pub struct MyModule {
    enabled: bool,
    x: f32,
    y: f32,
    node_ids: Vec<NodeId>,
}

impl OverlayModule for MyModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "my_module",
            name: "My Module",
            description: "Shows something useful",
        }
    }

    fn init(&mut self, scene: &mut Scene) {
        let id = scene.add_text(TextProps {
            x: self.x,
            y: self.y,
            text: "Hello, overlay".into(),
            font_size: 16.0,
            color: Color::WHITE,
        });
        self.node_ids.push(id);
    }

    fn update(&mut self, _scene: &mut Scene, _dt: Duration) -> bool {
        false
    }

    fn deinit(&mut self, scene: &mut Scene) {
        for id in self.node_ids.drain(..) {
            scene.remove(id);
        }
    }

    fn enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn set_position(&mut self, x: f32, y: f32) { self.x = x; self.y = y; }
    fn content_size(&self) -> (f32, f32) { (200.0, 20.0) }
}
```

## Related docs

- Running the reference app → [`getting-started.md`](getting-started.md)
- Building your own app → [`library-consumer.md`](library-consumer.md)
- High-level overview → [`../README.md`](../README.md)
