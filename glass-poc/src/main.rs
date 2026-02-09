//! GLASS PoC — thin harness exercising `glass-overlay`.
//!
//! Proves: wgpu DX12 + transparent HWND + click-through = viable overlay.
//! All core logic lives in `glass-overlay`; this binary is a bootstrap shim.
//!
//! Lifecycle:
//! 1. Init tracing (+ Tracy if `tracy` feature)
//! 2. Anti-cheat self-check (passive scan; blocks if kernel AC detected)
//! 3. DPI awareness
//! 4. Config load + hot-reload watcher
//! 5. Window + DComp + wgpu init
//! 6. Module registry setup (clock, system stats, FPS counter)
//! 7. Message loop (retained rendering + module ticks)
//!
//! Input modes: passive (default) ↔ interactive (hotkey toggle).
//! In test_mode builds, interactive mode is forcibly disabled.

mod alloc_tracker;

use glass_core::GlassError;
use glass_overlay::compositor::Compositor;
use glass_overlay::config::ConfigStore;
use glass_overlay::input::InputManager;
use glass_overlay::layout::{LayoutManager, WidgetWrapper};
use glass_overlay::modules::clock::ClockModule;
use glass_overlay::modules::fps_counter::FpsCounterModule;
use glass_overlay::modules::system_stats::SystemStatsModule;
use glass_overlay::overlay_window;
use glass_overlay::renderer::Renderer;
use glass_overlay::safety::{AntiCheatDetector, DetectionPolicy};
use tracing::{error, info, warn};

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

    // Debug-mode allocation tracking (Step 0.5)
    #[cfg(all(debug_assertions, feature = "alloc-tracking"))]
    alloc_tracker::install();

    info!("GLASS PoC starting");

    match run() {
        Ok(()) => info!("GLASS PoC exited cleanly"),
        Err(e) => {
            error!("GLASS PoC fatal: {e}");
            overlay_window::show_error_dialog("GLASS Fatal Error", &e.to_string());
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // ── Anti-cheat self-check (before any GPU / window init) ─────────────
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
            warn!("Anti-cheat self-check: user-mode AC detected ({names}) — proceeding with caution");
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
            cfg.input.hotkey_vk, cfg.input.hotkey_modifiers,
            cfg.input.interactive_timeout_ms, cfg.input.show_indicator
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

    // Start hot-reload watcher
    config_store.watch()?;

    // ── Window creation ─────────────────────────────────────────────────
    let hwnd = match overlay_window::create_overlay_window(
        input_cfg.interactive_timeout_ms,
        input_cfg.hotkey_vk,
        input_cfg.hotkey_modifiers,
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
            let msg = format!("DirectComposition init failed: {e}\n\nDWM composition may be disabled.");
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
            let msg = format!("GPU renderer init failed: {e}\n\nEnsure DX12-capable GPU drivers are installed.");
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
    let (screen_w, screen_h) = renderer.surface_dims();
    let mut layout_manager = LayoutManager::new(screen_w as f32, screen_h as f32);

    // Load layout config for per-widget anchor/margin settings
    let layout_cfg = {
        let cfg = config_store.get();
        cfg.layout.clone()
    };

    // Create widget wrappers with anchor-based positioning
    layout_manager.add_widget(WidgetWrapper::new(
        ClockModule::new(&modules_cfg.clock_format),
        layout_cfg.clock.anchor.clone(),
        layout_cfg.clock.margin_x,
        layout_cfg.clock.margin_y,
    ));
    layout_manager.add_widget(WidgetWrapper::new(
        SystemStatsModule::new(),
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

    // Apply module config (enable/disable per module settings)
    layout_manager.apply_config(&modules_cfg, renderer.scene_mut());

    // Initialize enabled modules (adds scene nodes at computed positions)
    layout_manager.init_all(renderer.scene_mut());

    info!(
        "Layout manager: {} widgets, {} enabled",
        layout_manager.len(),
        layout_manager.list().iter().filter(|(_, e)| *e).count()
    );

    // ── Demo interactive rect (development only) ────────────────────────
    unsafe {
        if let Some(state_ptr) = overlay_window::get_hwnd_input_state(hwnd) {
            // Demo interactive rect: a 200×60 button area at (100, 100)
            (*state_ptr).hit_tester.add_rect(100.0, 100.0, 200.0, 60.0, 0);
            info!("Demo interactive rect registered at (100,100) 200×60");
        }
    }

    // Input manager handles visual indicator lifecycle
    let mut input_manager = InputManager::new();

    // Initial render
    renderer.render()?;
    info!("Initial frame rendered");

    // ── Message loop (retained + module ticks) ──────────────────────────
    overlay_window::run_message_loop(&mut renderer, &mut input_manager, &mut layout_manager);

    // Deinit modules before exit
    layout_manager.deinit_all(renderer.scene_mut());
    info!("Modules deinitialized");

    Ok(())
}
