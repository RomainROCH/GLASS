//! GLASS Starter — reference implementation and starting point for custom GLASS overlays.
//!
//! This binary shows the intended consumer flow for a full GLASS application:
//! create the overlay window, initialize DirectComposition + the renderer,
//! register built-in modules, inject an external temperature callback for the
//! system stats module, enter the message loop, then deinitialize modules on
//! shutdown.
//!
//! Lifecycle:
//! 1. Init tracing (+ Tracy if `tracy` feature)
//! 2. Anti-cheat self-check (gaming builds only; passive scan; blocks if kernel AC detected)
//! 3. DPI awareness
//! 4. Config load + hot-reload watcher
//! 5. Window + DComp + wgpu init
//! 6. Layout manager + widget wrapper setup for built-in modules/widgets
//! 7. Message loop (retained rendering + module ticks)
//! 8. Clean shutdown via module deinit after the loop exits
//!
//! Input modes: passive (default) ↔ interactive (hotkey toggle).
//! In test_mode builds, interactive mode is forcibly disabled.

#[cfg(feature = "gaming")]
use glass_overlay::GlassError;
use glass_overlay::overlay_window;
#[cfg(feature = "gaming")]
use glass_overlay::safety::{AntiCheatDetector, DetectionPolicy};
use glass_overlay::{
    ClockModule, Compositor, ConfigStore, FpsCounterModule, InputManager, LayoutManager, Renderer,
    SystemStatsModule, WidgetWrapper,
};
#[cfg(feature = "gaming")]
use tracing::warn;
use tracing::{error, info};

/// Example external temperature callback used by the reference starter.
///
/// Replace this closure with your own hardware integration (for example:
/// a vendor SDK, a sensor daemon, or a local IPC/HTTP endpoint). GLASS does
/// not ship built-in temperature detection anymore; consumers inject it.
fn example_temp_source() -> Box<dyn FnMut() -> Option<f32> + Send> {
    Box::new(|| {
        // Placeholder integration point:
        // return `Some(temp_celsius)` when your app can read a temperature,
        // or `None` when no reading is available yet.
        None
    })
}

fn main() {
    // ── Tracing / logging ────────────────────────────────────────────────
    #[cfg(feature = "test_mode")]
    let default_filter = "trace";
    #[cfg(not(feature = "test_mode"))]
    let default_filter = "info";

    // Tracy subscriber: when the `tracy` feature is enabled, layer the
    // tracing-tracy subscriber on top of the fmt subscriber.
    #[cfg(feature = "tracy")]
    {
        use tracing_subscriber::layer::SubscriberExt;
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter));
        let tracy_layer = tracing_tracy::TracyLayer::default();
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .finish()
            .with(tracy_layer);
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set tracing subscriber");
    }

    #[cfg(not(feature = "tracy"))]
    {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
            )
            .init();
    }

    info!("GLASS Starter starting");

    match run() {
        Ok(()) => info!("GLASS Starter exited cleanly"),
        Err(e) => {
            error!("GLASS Starter fatal: {e}");
            overlay_window::show_error_dialog("GLASS Fatal Error", &e.to_string());
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // ── Anti-cheat self-check (gaming builds only) ───────────────────────
    #[cfg(feature = "gaming")]
    {
        let detector = AntiCheatDetector::new();
        let result = detector.scan();
        glass_overlay::safety::log_scan_result(&result);

        if result.should_block() {
            let names = result.blocked_names().join(", ");
            let msg = format!(
                "Kernel-level anti-cheat detected: {}.\n\n\
                 GLASS cannot run while kernel AC is active.\n\
                 Please close the anti-cheat software and try again.",
                names
            );
            error!("Anti-cheat gate: BLOCKING startup ({names})");
            overlay_window::show_error_dialog("GLASS — Anti-Cheat Detected", &msg);
            return Err(Box::new(GlassError::SafetyBlock(names)));
        }

        if result.has_warnings() {
            let names = result.warning_names().join(", ");
            warn!(
                "Anti-cheat self-check: user-mode AC detected ({names}) — proceeding with caution"
            );
        }

        for det in &result.detections {
            if det.policy == DetectionPolicy::Info {
                info!("Anti-cheat self-check: {}: info-only", det.system);
            }
        }
    }

    // ── DPI awareness — must be before any window creation ──────────────
    overlay_window::set_dpi_awareness();

    // ── Config load + hot-reload ────────────────────────────────────────
    let config_store = ConfigStore::load("config.ron")?;
    let (input_cfg, modules_cfg) = {
        let cfg = config_store.get();
        info!(
            "Config: position=({:.0},{:.0}), size=({:.0}x{:.0}), opacity={:.2}",
            cfg.position.x, cfg.position.y, cfg.size.width, cfg.size.height, cfg.opacity
        );
        info!(
            "Input config: hotkey_vk=0x{:02X}, mods=0x{:X}, timeout={}ms, indicator={}",
            cfg.input.hotkey_vk,
            cfg.input.hotkey_modifiers,
            cfg.input.interactive_timeout_ms,
            cfg.input.show_indicator
        );
        info!(
            "Modules config: clock={}, stats={} ({}ms), fps={}",
            cfg.modules.clock_enabled,
            cfg.modules.system_stats_enabled,
            cfg.modules.stats_interval_ms,
            cfg.modules.fps_enabled,
        );
        (cfg.input.clone(), cfg.modules.clone())
    };

    // Start watching the config file so a consumer can later re-read and
    // reapply settings after edits.
    config_store.watch()?;

    // ── Window creation ─────────────────────────────────────────────────
    let hwnd = match overlay_window::create_overlay_window(
        input_cfg.interactive_timeout_ms,
        input_cfg.hotkey_vk,
        input_cfg.hotkey_modifiers,
        "GLASS",
    ) {
        Ok(h) => h,
        Err(e) => {
            let msg = format!("Failed to create overlay window: {e}");
            error!("{msg}");
            overlay_window::show_error_dialog("GLASS — Window Error", &msg);
            return Err(Box::new(e));
        }
    };
    info!("Overlay window created");

    // ── DirectComposition ───────────────────────────────────────────────
    let dcomp = match Compositor::new(hwnd) {
        Ok(d) => d,
        Err(e) => {
            let msg =
                format!("DirectComposition init failed: {e}\n\nDWM composition may be disabled.");
            error!("{msg}");
            overlay_window::show_error_dialog("GLASS — DComp Error", &msg);
            return Err(Box::new(e));
        }
    };
    info!("DirectComposition compositor ready");

    // ── wgpu DX12 renderer ──────────────────────────────────────────────
    let mut renderer = match Renderer::new(dcomp.visual_handle(), hwnd) {
        Ok(r) => r,
        Err(e) => {
            let msg = format!(
                "GPU renderer init failed: {e}\n\nEnsure DX12-capable GPU drivers are installed."
            );
            error!("{msg}");
            overlay_window::show_error_dialog("GLASS — Renderer Error", &msg);
            return Err(Box::new(e));
        }
    };
    info!("wgpu DX12 renderer initialized");

    // Commit DComp: makes the visual → swapchain binding take effect
    dcomp.commit()?;
    info!("DComp committed");

    // ── Layout manager (anchor-based widget positioning) ────────────────
    // The layout manager owns the module list and keeps each widget anchored
    // to the configured screen position.
    let (screen_w, screen_h) = renderer.surface_dims();
    let mut layout_manager = LayoutManager::new(screen_w as f32, screen_h as f32);

    // Load layout config for per-widget anchor/margin settings
    let layout_cfg = {
        let cfg = config_store.get();
        cfg.layout.clone()
    };

    // Build the clock module and place it with the configured anchor/margins.
    layout_manager.add_widget(WidgetWrapper::new(
        ClockModule::new(&modules_cfg.clock_format),
        layout_cfg.clock.anchor.clone(),
        layout_cfg.clock.margin_x,
        layout_cfg.clock.margin_y,
    ));

    // Build the system stats module, then inject an external temperature
    // callback. This starter intentionally uses a placeholder closure so the
    // example stays honest: GLASS itself is sensor-library agnostic.
    let mut system_stats = SystemStatsModule::new();
    system_stats.set_temp_source(example_temp_source());
    layout_manager.add_widget(WidgetWrapper::new(
        system_stats,
        layout_cfg.system_stats.anchor.clone(),
        layout_cfg.system_stats.margin_x,
        layout_cfg.system_stats.margin_y,
    ));

    // Add the FPS module so the reference app demonstrates multiple built-in
    // modules sharing the same layout/message-loop infrastructure.
    layout_manager.add_widget(WidgetWrapper::new(
        FpsCounterModule::new(),
        layout_cfg.fps.anchor.clone(),
        layout_cfg.fps.margin_x,
        layout_cfg.fps.margin_y,
    ));

    // Apply enable/disable settings from config before initialization so the
    // scene only contains widgets the user asked for.
    layout_manager.apply_config(&modules_cfg, renderer.scene_mut());

    // Initialize enabled modules. Each module adds its scene nodes here using
    // the layout-computed position supplied by the wrapper.
    layout_manager.init_all(renderer.scene_mut());

    info!(
        "Layout manager: {} widgets, {} enabled",
        layout_manager.len(),
        layout_manager.list().iter().filter(|(_, e)| *e).count()
    );

    // The input manager owns the passive/interactive indicator visuals.
    let mut input_manager = InputManager::new();

    // Draw the initial retained scene before we start processing messages.
    renderer.render()?;
    info!("Initial frame rendered");

    // ── Message loop (retained + module ticks) ──────────────────────────
    // This blocks until the user exits via the tray icon or the window is
    // otherwise asked to quit.
    overlay_window::run_message_loop(&mut renderer, &mut input_manager, &mut layout_manager);

    // Clean shutdown: give every module a chance to remove its scene nodes and
    // release any module-owned state before the process exits.
    layout_manager.deinit_all(renderer.scene_mut());
    info!("Modules deinitialized");

    Ok(())
}
