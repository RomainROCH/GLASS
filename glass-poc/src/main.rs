//! GLASS PoC — thin harness exercising `glass-overlay`.
//!
//! Proves: wgpu DX12 + transparent HWND + click-through = viable overlay.
//! All core logic lives in `glass-overlay`; this binary is a bootstrap shim.

mod alloc_tracker;

use glass_overlay::compositor::Compositor;
use glass_overlay::overlay_window;
use glass_overlay::renderer::Renderer;
use tracing::{error, info};

fn main() {
    // Tracing / logging
    #[cfg(feature = "test_mode")]
    let default_filter = "trace";
    #[cfg(not(feature = "test_mode"))]
    let default_filter = "info";

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
        )
        .init();

    // Debug-mode allocation tracking (Step 0.5)
    #[cfg(all(debug_assertions, feature = "alloc-tracking"))]
    alloc_tracker::install();

    info!("GLASS PoC starting");

    match run() {
        Ok(()) => info!("GLASS PoC exited cleanly"),
        Err(e) => error!("GLASS PoC fatal: {e}"),
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // DPI awareness — must be first
    overlay_window::set_dpi_awareness();

    // Create the overlay HWND
    let hwnd = overlay_window::create_overlay_window()?;
    info!("Overlay window created");

    // DirectComposition: creates device + target + visual
    let dcomp = Compositor::new(hwnd)?;
    info!("DirectComposition compositor ready");

    // Init wgpu DX12 — binds swap chain to the DComp visual
    let mut renderer = Renderer::new(dcomp.visual_handle(), hwnd)?;
    info!("wgpu DX12 renderer initialized");

    // Commit DComp: makes the visual → swapchain binding take effect
    dcomp.commit()?;
    info!("DComp committed");

    // Initial render
    renderer.render()?;
    info!("Initial frame rendered");

    // Message loop — retained: only re-render on WM_PAINT / WM_SIZE
    overlay_window::run_message_loop(&mut renderer);

    Ok(())
}
