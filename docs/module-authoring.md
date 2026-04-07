---
created: 2026-03-26
updated: 2026-04-07
category: user
status: active
doc_kind: guide
---

# Module authoring

GLASS modules are small units of overlay behavior that own their own scene nodes and plug into layout through `WidgetWrapper`.

## The `OverlayModule` contract

Custom modules implement `OverlayModule`:

```rust
use glass_overlay::{ModuleInfo, OverlayModule, Scene};
use std::time::Duration;
```

The key lifecycle is:

- `init(scene)` -> create your scene nodes
- `update(scene, dt)` -> change those nodes when your data changes
- `deinit(scene)` -> remove the nodes you created

Layout also calls:

- `set_position(x, y)` -> receive the computed screen position
- `content_size()` -> report your estimated width and height

## Scene graph API

Modules draw by mutating the retained `Scene`.

```rust
use glass_overlay::{Color, Scene, SceneNode, TextProps};

fn edit_scene(scene: &mut Scene) {
    let id = scene.add_text(TextProps {
        x: 20.0,
        y: 20.0,
        text: "Hello".into(),
        font_size: 18.0,
        color: Color::WHITE,
    });

    scene.update(
        id,
        SceneNode::Text(TextProps {
            x: 20.0,
            y: 20.0,
            text: "Updated".into(),
            font_size: 18.0,
            color: Color::WHITE,
        }),
    );

    scene.remove(id);
}
```

Use:

- `Scene::add_text(...)` / `Scene::add_rect(...)` to create nodes
- `Scene::update(id, SceneNode::...)` to replace a node's contents
- `Scene::remove(id)` to delete nodes during cleanup

## Layout system

`WidgetWrapper` connects your module to the layout system:

```rust
use glass_overlay::{Anchor, ClockModule, LayoutManager, WidgetWrapper};

fn build_layout() -> LayoutManager {
    let mut layout_manager = LayoutManager::new(1920.0, 1080.0);
    layout_manager.add_widget(WidgetWrapper::new(
        ClockModule::new("%H:%M:%S"),
        Anchor::TopRight,
        20.0,
        20.0,
    ));
    layout_manager
}
```

Important pieces:

- **anchors**: `TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`, `Center`, `ScreenPercentage(x, y)`
- **margins**: offsets from the anchor point
- **`content_size()`**: the size layout uses to resolve the final top-left position and hit-test bounds

If your module's size estimate is too small, hit-testing and anchor placement can feel wrong, so return a conservative bounding box.

## Config integration

GLASS ships typed config for overlay runtime settings and the built-in modules. Custom module settings stay in your application layer.

Typical pattern:

1. load `OverlayConfig` with `ConfigStore` if you want GLASS's built-in config
2. keep your own app-specific config for custom module data
3. pass your custom settings into the module constructor and `WidgetWrapper::new(...)`

Example:

```rust
use glass_overlay::Anchor;

struct TickerConfig {
    text: String,
    anchor: Anchor,
    margin_x: f32,
    margin_y: f32,
}
```

If you also watch config files, remember that `ConfigStore::watch()` only refreshes the stored snapshot. Your app must decide when and how to re-read and reapply updated settings.

## Complete example: text ticker module

This example compiles against the current API and shows add/update/remove behavior.

```rust
use glass_overlay::{
    Anchor, Color, LayoutManager, ModuleInfo, NodeId, OverlayModule, Scene, SceneNode, TextProps,
    WidgetWrapper,
};
use std::time::Duration;

pub struct TickerModule {
    enabled: bool,
    x: f32,
    y: f32,
    node_id: Option<NodeId>,
    message: String,
    step_accumulator: Duration,
    offset: usize,
}

impl TickerModule {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            enabled: true,
            x: 0.0,
            y: 0.0,
            node_id: None,
            message: message.into(),
            step_accumulator: Duration::ZERO,
            offset: 0,
        }
    }

    fn rendered_text(&self) -> String {
        let chars: Vec<char> = self.message.chars().collect();
        if chars.is_empty() {
            return String::new();
        }

        let mut out = String::with_capacity(chars.len());
        for i in 0..chars.len() {
            out.push(chars[(self.offset + i) % chars.len()]);
        }
        out
    }
}

impl OverlayModule for TickerModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "ticker",
            name: "Ticker",
            description: "Rotating single-line text ticker",
        }
    }

    fn init(&mut self, scene: &mut Scene) {
        let id = scene.add_text(TextProps {
            x: self.x,
            y: self.y,
            text: self.rendered_text(),
            font_size: 18.0,
            color: Color::WHITE,
        });
        self.node_id = Some(id);
    }

    fn update(&mut self, scene: &mut Scene, dt: Duration) -> bool {
        let char_count = self.message.chars().count();
        if char_count == 0 {
            return false;
        }

        self.step_accumulator += dt;
        if self.step_accumulator < Duration::from_millis(250) {
            return false;
        }
        self.step_accumulator = Duration::ZERO;
        self.offset = (self.offset + 1) % char_count;

        if let Some(id) = self.node_id {
            scene.update(
                id,
                SceneNode::Text(TextProps {
                    x: self.x,
                    y: self.y,
                    text: self.rendered_text(),
                    font_size: 18.0,
                    color: Color::WHITE,
                }),
            )
        } else {
            false
        }
    }

    fn deinit(&mut self, scene: &mut Scene) {
        if let Some(id) = self.node_id.take() {
            scene.remove(id);
        }
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn content_size(&self) -> (f32, f32) {
        (320.0, 24.0)
    }
}

fn register_ticker(layout_manager: &mut LayoutManager) {
    layout_manager.add_widget(WidgetWrapper::new(
        TickerModule::new("GLASS says hello"),
        Anchor::BottomRight,
        20.0,
        20.0,
    ));
}
```

After registration, initialize and later clean up through the layout manager:

```rust
use glass_overlay::{LayoutManager, Renderer};

fn start_and_stop(
    renderer: &mut Renderer,
    layout_manager: &mut LayoutManager,
) {
    layout_manager.init_all(renderer.scene_mut());
    layout_manager.deinit_all(renderer.scene_mut());
}
```

## Related docs

- Run the reference app: [`getting-started.md`](getting-started.md)
- Build your own app: [`library-consumer.md`](library-consumer.md)
- Project overview: [`../README.md`](../README.md)
