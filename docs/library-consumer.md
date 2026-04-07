---
created: 2026-03-26
updated: 2026-04-07
category: user
status: active
doc_kind: guide
---

# Library consumer guide

Use this guide when you want to build your own Windows overlay application on top of GLASS.

## Requirements

- Windows 10 or Windows 11
- DirectComposition-capable desktop composition
- DX12-capable graphics stack
- Rust stable 1.85+

GLASS is currently Windows-only because the runtime depends on Win32 APIs, DirectComposition, and a DX12 `wgpu` surface path.

## 1. Add dependencies

`glass-overlay` is the main library crate. `glass-core` is available if you want to depend on shared core types directly, although `glass-overlay` already re-exports `GlassError`.

```toml
[dependencies]
glass-core = { path = "../glass-core" }
glass-overlay = { path = "../glass-overlay" }
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

Prefer crate-root imports instead of deep module paths:

```rust
use glass_overlay::{
    overlay_window, ClockModule, Compositor, ConfigStore, FpsCounterModule, InputManager,
    LayoutManager, Renderer, SystemStatsModule, WidgetWrapper,
};
```

## 2. Create the window, renderer, and layout manager

The current reference flow is:

1. set DPI awareness
2. load config
3. create the overlay window
4. create `Compositor`
5. create `Renderer`
6. commit DirectComposition
7. create `LayoutManager`
8. add modules
9. render once and run the message loop

This example matches the current crate-root API and the starter's flow:

```rust
use glass_overlay::{
    overlay_window, ClockModule, Compositor, ConfigStore, FpsCounterModule, InputManager,
    LayoutManager, Renderer, SystemStatsModule, WidgetWrapper,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    overlay_window::set_dpi_awareness();

    let config_store = ConfigStore::load("config.ron")?;
    let (input_cfg, modules_cfg, layout_cfg) = {
        let cfg = config_store.get();
        (cfg.input.clone(), cfg.modules.clone(), cfg.layout.clone())
    };
    config_store.watch()?;

    let hwnd = overlay_window::create_overlay_window(
        input_cfg.interactive_timeout_ms,
        input_cfg.hotkey_vk,
        input_cfg.hotkey_modifiers,
        "My GLASS App",
    )?;

    let dcomp = Compositor::new(hwnd)?;
    let mut renderer = Renderer::new(dcomp.visual_handle(), hwnd)?;
    dcomp.commit()?;

    let (screen_w, screen_h) = renderer.surface_dims();
    let mut layout_manager = LayoutManager::new(screen_w as f32, screen_h as f32);

    layout_manager.add_widget(WidgetWrapper::new(
        ClockModule::new(&modules_cfg.clock_format),
        layout_cfg.clock.anchor.clone(),
        layout_cfg.clock.margin_x,
        layout_cfg.clock.margin_y,
    ));

    let mut system_stats = SystemStatsModule::new();
    system_stats.set_temp_source(Box::new(|| None::<f32>));
    layout_manager.add_widget(WidgetWrapper::new(
        system_stats,
        layout_cfg.system_stats.anchor.clone(),
        layout_cfg.system_stats.margin_x,
        layout_cfg.system_stats.margin_y,
    ));

    layout_manager.add_widget(WidgetWrapper::new(
        FpsCounterModule::new(),
        layout_cfg.fps.anchor.clone(),
        layout_cfg.fps.margin_x,
        layout_cfg.fps.margin_y,
    ));

    layout_manager.apply_config(&modules_cfg, renderer.scene_mut());
    layout_manager.init_all(renderer.scene_mut());

    let mut input_manager = InputManager::new();
    renderer.render()?;
    overlay_window::run_message_loop(&mut renderer, &mut input_manager, &mut layout_manager);
    layout_manager.deinit_all(renderer.scene_mut());

    Ok(())
}
```

For the absolute minimum setup, inspect [`../glass-starter/examples/minimal.rs`](../glass-starter/examples/minimal.rs).

## 3. Add modules

Modules are usually added through `WidgetWrapper`, which combines:

- an `OverlayModule`
- an `Anchor`
- horizontal and vertical margins

`LayoutManager` uses the module's `content_size()` plus the wrapper's anchor and margins to compute its screen position.

## 4. Temperature callback pattern

`SystemStatsModule` no longer performs built-in temperature detection. Consumers inject their own source with `set_temp_source()`.

```rust
use glass_overlay::SystemStatsModule;

fn make_temp_source() -> Box<dyn FnMut() -> Option<f32> + Send> {
    Box::new(|| {
        // Replace this with your own sensor integration.
        // Return Some(temp_celsius) when available, or None when unavailable.
        None::<f32>
    })
}

fn build_stats_module() -> SystemStatsModule {
    let mut stats = SystemStatsModule::new();
    stats.set_temp_source(make_temp_source());
    stats
}
```

This keeps GLASS sensor-library agnostic. Your callback can read from a vendor SDK, a local service, shared memory, IPC, or any other source you control.

## 5. `OverlayModule` lifecycle

Every module implements `OverlayModule`.

| Method | When it runs | Typical responsibility |
|---|---|---|
| `init(scene)` | When the module is initialized or re-enabled | Add scene nodes |
| `update(scene, dt)` | On message-loop ticks while enabled | Refresh node content and return `true` if you changed the scene |
| `deinit(scene)` | On shutdown or disable | Remove scene nodes |
| `set_position(x, y)` | When layout computes coordinates | Store the layout position |
| `content_size()` | During layout and hit-testing | Report an estimated width and height |

If your module creates nodes in `init`, it should remove them in `deinit`.

## 6. Run the overlay

Once the scene is initialized:

```rust
use glass_overlay::{overlay_window, InputManager, LayoutManager, Renderer};

fn run_loop(
    renderer: &mut Renderer,
    layout_manager: &mut LayoutManager,
) -> Result<(), glass_overlay::GlassError> {
    let mut input_manager = InputManager::new();
    renderer.render()?;
    overlay_window::run_message_loop(renderer, &mut input_manager, layout_manager);
    layout_manager.deinit_all(renderer.scene_mut());
    Ok(())
}
```

`run_message_loop` blocks until the overlay exits.

## Configuration notes

- `ConfigStore::load("config.ron")` loads RON
- `ConfigStore::load("config.toml")` loads TOML
- `ConfigStore::watch()` updates the stored snapshot on file changes

The reference starter watches config changes but does not currently re-read and reapply them after startup.

## Related docs

- First run: [`getting-started.md`](getting-started.md)
- Custom modules: [`module-authoring.md`](module-authoring.md)
- Project overview: [`../README.md`](../README.md)
