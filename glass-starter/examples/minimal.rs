//! Minimal GLASS overlay bootstrap.
//!
//! Demonstrates the bare minimum to get a transparent, click-through
//! DirectComposition overlay window running. No modules, no config,
//! no layout — just a clear window you can draw into.
//!
//! Run with:
//! ```sh
//! cargo run --example minimal -p glass-starter
//! ```

use glass_overlay::compositor::Compositor;
use glass_overlay::input::InputManager;
use glass_overlay::layout::LayoutManager;
use glass_overlay::overlay_window;
use glass_overlay::renderer::Renderer;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    match run() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error: {e}");
            overlay_window::show_error_dialog("GLASS Error", &e.to_string());
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // DPI awareness — must come before window creation
    overlay_window::set_dpi_awareness();

    // Create the overlay window (fullscreen, topmost, click-through).
    // Pass zero for timeout/hotkey to disable interactive mode.
    let hwnd = overlay_window::create_overlay_window(0, 0, 0)
        .map_err(|e| format!("Window: {e}"))?;

    // DirectComposition compositor
    let dcomp = Compositor::new(hwnd)
        .map_err(|e| format!("DComp: {e}"))?;

    // wgpu DX12 renderer
    let mut renderer = Renderer::new(dcomp.visual_handle(), hwnd)
        .map_err(|e| format!("Renderer: {e}"))?;
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

    // Commit DComp (binds visual → swapchain)
    dcomp.commit()?;

    // Render first frame
    renderer.render()?;

    // Run message loop (handles WM_QUIT, resize, tray icon exit)
    let mut input_manager = InputManager::new();
    let (w, h) = renderer.surface_dims();
    let mut layout_manager = LayoutManager::new(w as f32, h as f32);
    overlay_window::run_message_loop(&mut renderer, &mut input_manager, &mut layout_manager);

    Ok(())
}
