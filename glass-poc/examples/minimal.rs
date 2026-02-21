//! Minimal standalone example — transparent overlay using existing APIs.
//!
//! Demonstrates the essential bootstrap sequence without PoC-specific features:
//! 1. Tracing init
//! 2. DPI awareness
//! 3. Window creation
//! 4. DirectComposition init
//! 5. wgpu Renderer init
//! 6. Commit compositor
//! 7. Initial render
//! 8. Message loop
//!
//! Omits: anti-cheat checks, config loading/hot-reload, module registry,
//! layout system, demo interactive rects.

use glass_overlay::compositor::Compositor;
use glass_overlay::input::InputManager;
use glass_overlay::layout::LayoutManager;
use glass_overlay::overlay_window;
use glass_overlay::renderer::Renderer;
use tracing::{error, info};

fn main() {
    // ── Tracing init ─────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("GLASS minimal example starting");

    match run() {
        Ok(()) => info!("Example exited cleanly"),
        Err(e) => {
            error!("Fatal error: {e}");
            overlay_window::show_error_dialog("GLASS Minimal Example Error", &e.to_string());
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // ── DPI awareness (before any window creation) ───────────────────────
    overlay_window::set_dpi_awareness();

    // ── Window creation ──────────────────────────────────────────────────
    // Use default hotkey (F12) and timeout (3000ms) for demonstration
    let hwnd = overlay_window::create_overlay_window(3000, 0x7B, 0)?;
    info!("Overlay window created");

    // ── DirectComposition ────────────────────────────────────────────────
    let dcomp = Compositor::new(hwnd)?;
    info!("DirectComposition compositor ready");

    // ── wgpu DX12 renderer ───────────────────────────────────────────────
    let mut renderer = Renderer::new(dcomp.visual_handle(), hwnd)?;
    info!("wgpu DX12 renderer initialized");

    // Commit DComp: makes the visual → swapchain binding take effect
    dcomp.commit()?;
    info!("DComp committed");

    // ── Empty input and layout managers ──────────────────────────────────
    // Required by run_message_loop signature, but no widgets/modules added
    let mut input_manager = InputManager::new();
    let (screen_w, screen_h) = renderer.surface_dims();
    let mut layout_manager = LayoutManager::new(screen_w as f32, screen_h as f32);

    // ── Initial render ───────────────────────────────────────────────────
    renderer.render()?;
    info!("Initial frame rendered");

    // ── Message loop ─────────────────────────────────────────────────────
    info!("Entering message loop");
    overlay_window::run_message_loop(&mut renderer, &mut input_manager, &mut layout_manager);

    Ok(())
}
