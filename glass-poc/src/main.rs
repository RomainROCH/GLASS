//! GLASS Phase 0 — Proof of Concept
//!
//! Proves: wgpu DX12 + transparent HWND + click-through = viable overlay.
//! Constraints: < 500 LOC, no UI, no input beyond passthrough.

// ── Modules ──────────────────────────────────────────────────────────────────
mod alloc_tracker;
mod compositor;
mod overlay_window;
mod renderer;

use tracing::{error, info};

fn main() {
    // Tracing / logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Debug-mode allocation tracking (Step 0.5)
    #[cfg(all(debug_assertions, feature = "alloc-tracking"))]
    alloc_tracker::install();

    info!("GLASS PoC starting");

    // Step 0.2 + 0.3 + 0.4: Create window, init wgpu, render triangle
    match run() {
        Ok(()) => info!("GLASS PoC exited cleanly"),
        Err(e) => error!("GLASS PoC fatal: {e}"),
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // DPI awareness — must be first (Step 0.2)
    overlay_window::set_dpi_awareness();

    // Create the overlay HWND (Step 0.2 + 0.4)
    let hwnd = overlay_window::create_overlay_window()?;
    info!("Overlay window created");

    // DirectComposition: creates device + target + visual
    let dcomp = compositor::Compositor::new(hwnd)?;
    info!("DirectComposition compositor ready");

    // Init wgpu DX12 — binds swap chain to the DComp visual
    let mut renderer = renderer::Renderer::new(dcomp.visual_handle(), hwnd)?;
    info!("wgpu DX12 renderer initialized");

    // Commit DComp: makes the visual → swapchain binding take effect
    dcomp.commit()?;
    info!("DComp committed");

    // Initial render — green triangle (Step 0.3)
    renderer.render()?;
    info!("Initial frame rendered");

    // Message loop — retained: only re-render on WM_PAINT
    overlay_window::run_message_loop(&mut renderer);

    Ok(())
}
