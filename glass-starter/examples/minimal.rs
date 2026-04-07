//! Minimal GLASS overlay bootstrap.
//!
//! Demonstrates the smallest honest integration against the current
//! `glass-overlay` API: create the overlay window, initialize
//! DirectComposition, set up the renderer, render once, run the message loop,
//! and exit cleanly. It intentionally skips config loading and built-in modules.
//!
//! Run with:
//! ```sh
//! cargo run --example minimal -p glass-starter
//! ```

use glass_overlay::overlay_window;
use glass_overlay::{Compositor, GlassError, InputManager, LayoutManager, Renderer};

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    match run() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error: {e}");
            overlay_window::show_error_dialog("GLASS Error", &e.to_string());
        }
    }
}

fn run() -> Result<(), GlassError> {
    // Step 1: opt into PerMonitorAwareV2 DPI handling before creating a HWND.
    overlay_window::set_dpi_awareness();

    // Step 2: create the transparent overlay window. Passing zeroes disables
    // the optional interactive-mode hotkey for this tiny example.
    let hwnd = overlay_window::create_overlay_window(0, 0, 0, "GLASS")
        .map_err(|e| GlassError::WindowCreation(e.to_string()))?;

    // Step 3: create the DirectComposition objects that own the visual tree.
    let dcomp = Compositor::new(hwnd).map_err(|e| GlassError::CompositionInit(e.to_string()))?;

    // Step 4: bind the renderer to the compositor visual and the overlay HWND.
    let mut renderer = Renderer::new(dcomp.visual_handle(), hwnd)
        .map_err(|e| GlassError::WgpuInit(e.to_string()))?;
    // Example: draw a triangle — replace with your own render logic
    // const SHADER_SRC: &str = r#"
    // struct VertexOutput {
    //     @builtin(position) position: vec4<f32>,
    //     @location(0) color: vec4<f32>,
    // };
    //
    // @vertex
    // fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    //     var positions = array<vec2<f32>, 3>(
    //         vec2<f32>( 0.0,  0.5),
    //         vec2<f32>(-0.5, -0.5),
    //         vec2<f32>( 0.5, -0.5),
    //     );
    //     var out: VertexOutput;
    //     out.position = vec4<f32>(positions[idx], 0.0, 1.0);
    //     out.color = vec4<f32>(0.0, 0.5, 0.0, 0.5);
    //     return out;
    // }
    //
    // @fragment
    // fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    //     return in.color;
    // }
    // "#;

    // Step 5: commit the DirectComposition tree so the swapchain becomes visible.
    dcomp.commit()?;

    // Step 6: render the initial transparent frame.
    renderer.render()?;

    // Step 7: create the minimal input/layout state required by the current
    // message-loop API, then block until the overlay exits.
    let mut input_manager = InputManager::new();
    let (w, h) = renderer.surface_dims();
    let mut layout_manager = LayoutManager::new(w as f32, h as f32);
    overlay_window::run_message_loop(&mut renderer, &mut input_manager, &mut layout_manager);

    // Step 8: returning cleanly hands control back to `main`, which reports success.
    Ok(())
}
