---
created: 2026-03-26
updated: 2026-03-26
category: user
status: active
doc_kind: guide
---

# Library consumer guide

Use this page when you want to build your own overlay application on top of `glass-overlay`.

## Dependency snippet

For a local checkout/workspace-style integration:

```toml
[dependencies]
glass-overlay = { path = "../glass-overlay" }
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

## Recommended root imports

Prefer crate-root re-exports over deep module imports:

```rust
use glass_overlay::{
    overlay_window, Compositor, ConfigStore, InputManager, LayoutManager, Renderer,
    WidgetWrapper,
};
```

Add scene or module types from the crate root as needed:

```rust
use glass_overlay::{Anchor, Color, OverlayModule, Scene, TextProps};
```

## Minimal bootstrap sequence

The normal app bootstrap is:

1. initialize tracing
2. call `overlay_window::set_dpi_awareness()`
3. load config with `ConfigStore::load(...)`
4. create the overlay window
5. create `Compositor`
6. create `Renderer`
7. commit DirectComposition
8. create `LayoutManager` and register widgets/modules
9. create `InputManager`
10. render once, then enter the message loop

Minimal skeleton:

```rust
use glass_overlay::{
    overlay_window, Compositor, ConfigStore, InputManager, LayoutManager, Renderer,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    overlay_window::set_dpi_awareness();

    let config_store = ConfigStore::load("config.ron")?;
    config_store.watch()?;

    let cfg = config_store.get();
    let hwnd = overlay_window::create_overlay_window(
        cfg.input.interactive_timeout_ms,
        cfg.input.hotkey_vk,
        cfg.input.hotkey_modifiers,
        "My GLASS App",
    )?;

    let dcomp = Compositor::new(hwnd)?;
    let mut renderer = Renderer::new(dcomp.visual_handle(), hwnd)?;
    dcomp.commit()?;

    let (w, h) = renderer.surface_dims();
    let mut layout_manager = LayoutManager::new(w as f32, h as f32);
    let mut input_manager = InputManager::new();

    renderer.render()?;
    overlay_window::run_message_loop(&mut renderer, &mut input_manager, &mut layout_manager);
    Ok(())
}
```

For the absolute smallest bootstrap, inspect [`../glass-starter/examples/minimal.rs`](../glass-starter/examples/minimal.rs).

## Config format choice

`glass-starter` loads `config.ron` by default. Your own app is not limited to that.

`ConfigStore` accepts either `.ron` or `.toml` based on the path you pass:

```rust
let ron_store = ConfigStore::load("config.ron")?;
let toml_store = ConfigStore::load("config.toml")?;
```

## When to use `glass-starter` vs `glass-overlay`

| Use | Choose |
|-----|--------|
| You want a working reference app immediately | `glass-starter` |
| You want to study the intended full wiring | `glass-starter` |
| You want your own binary, config path, modules, or startup flow | `glass-overlay` |
| You want the smallest embed example | `glass-starter/examples/minimal.rs` |

## Related docs

- First run overview → [`getting-started.md`](getting-started.md)
- Module registration and lifecycle → [`module-authoring.md`](module-authoring.md)
- High-level architecture → [`../README.md`](../README.md)
