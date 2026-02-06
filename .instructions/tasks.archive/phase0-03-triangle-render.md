---
schema: task/v1
id: task-000001
title: "Render triangle: 50% alpha green triangle via wgpu, retained rendering, Tracy spans"
type: feature
status: archived
priority: medium
owner: "executive2"
skills: ["frontend", "design"]
depends_on: []
next_tasks: []
created: "2026-02-06"
updated: "2026-02-06"
---

## Goal ✅
Render a single, green triangle with 50% alpha using wgpu and *premultiplied* alpha blending. The renderer must be retained (no continuous redraws — draw only on initial present or when explicitly invalidated). Add Tracy profiling spans for `acquire`, `render`, `present`, and `commit` so the full frame lifecycle is measurable.

---

## Context 🔧
- This is Phase0 visual verification work for the overlay renderer.
- No existing `wgpu`-based renderer found in repo; this is intended as a small, self-contained proof-of-concept renderer.  
- Relevant local files/notes:
  - `.instructions/architecture.md` — repository/architecture context
  - `.instructions/contexts/project.index.md` — project overview
- External references:
  - wgpu docs / blend states (premultiplied alpha): https://wgpu.rs/ and API docs
  - Premultiplied alpha explanation: https://en.wikipedia.org/wiki/Alpha_compositing#Premultiplied_alpha
  - Tracy profiler usage: https://github.com/wolfpld/tracy

---

## Acceptance Criteria ✅
1. Visual correctness
   - A single filled triangle is visible on app start.
   - Triangle color is green with 50% alpha; **premultiplied** source color must be used by the pipeline.
   - On a white background, sampled triangle center pixel should be approximately RGB(128,255,128) (i.e., (0.5,1.0,0.5) in normalized color — allow small rounding/tone differences from sRGB conversion).
2. Blending configuration
   - Pipeline uses premultiplied alpha blending for color (src factor = One, dst factor = OneMinusSrcAlpha) and an appropriate alpha blend that preserves expected alpha.
3. Retained rendering
   - Renderer does not continuously redraw. After the initial frame, no additional `render` work (GPU draws or render pass submission) occurs unless an explicit invalidation occurs (e.g., window resize, content change, or manual invalidate call).
4. Tracy spans
   - Trace frames include named Tracy spans (or zone markers): `acquire`, `render`, `present`, `commit`.
   - Spans are measurable and appear in a captured trace in the expected order (acquire → render → present → commit). Each frame's spans are visible and have non-zero durations.
5. Tests / validation hooks
   - Provide an easy validation path (manual steps are fine): e.g., a command-line flag or UI toggle to present a white/black background and capture a screenshot / pixel sample for automated verification.
6. Implementation quality
   - Implementation is small, well-documented, and includes at least one integration or manual validation note added under `Validation Notes` below.

---

## Plan / Approach 🛠️
1. Add a small renderer module / component (e.g.`renderer/triangle_renderer`) that:
   - Initializes `wgpu` surface / device / queue and creates a render pipeline.
   - Uses a simple triangle vertex buffer and a fragment shader that outputs green with 50% alpha.
   - Configures the pipeline's BlendState for premultiplied alpha:
     - color: src = One, dst = OneMinusSrcAlpha, operation = Add
     - alpha: src = One, dst = OneMinusSrcAlpha (or other appropriate alpha blend)
2. Implement retained rendering semantics:
   - Render once to the surface and present. Do not schedule continuous redraws.
   - Provide an `invalidate()` API that triggers another render (to support resizing / content updates).
3. Add Tracy instrumentation:
   - Surround lifecycle stages with Tracy zones/spans named `acquire`, `render`, `present`, and `commit`.
   - Ensure spans are emitted even in the retained case (to make sure single-frame cases can still be profiled).
4. Add manual validation hooks (dev flag or runtime toggle) for background selection (white/black) and optional auto-screenshot capability for pixel asserts.
5. Add minimal tests or manual verification steps to `Validation Notes`.

---

## Validation Notes 🧪
Manual validation steps (quick):
1. Build & run the app in the target environment with Tracy enabled.
2. Start with a white background, start the app, then capture a screenshot or sample the center pixel of the triangle. Expected approx: RGB(128,255,128) (premultiplied composited over white).
3. Repeat with a black background; expected approx: RGB(0,128,0) (50% green over black). These values are approximate and should tolerate colorspace/sRGB conversion issues; document tolerances in the test code.
4. Capture a Tracy profile & verify spans are present and ordered for the initial frame: `acquire`, `render`, `present`, `commit`.
5. Verify retained behavior: after initial render completes, verify (via logs, counters, or Tracy) that no additional `render` spans appear until you call `invalidate()`.
6. Programmatic check (optional): add a debug-mode counter that increments when `render` span occurs; assert it stays at `1` after startup.

---

## Notes / Risks ⚠️
- Premultiplied alpha blending must be enforced both in the fragment output (if feeding premultiplied color) and in pipeline blending config. Mismatch will yield incorrect visual results.
- If the platform's surface/resizing model expects continuous redraws, retained behavior may need platform-specific handling; call this out in code comments and link to platform surface docs as needed.
- Tracy integration must match the project's existing profiling approach; if the project uses a different trace naming convention, adapt accordingly.

---

## Next Steps ▶️
- Confirm owner (replace `"<dev-handle>"` with assignee).  
- Implement small renderer as described, add validation hooks and a short README snippet showing how to run and verify with Tracy.  
- Optional follow-ups: add automated pixel-compare test to CI, extend to additional colors/shapes.

---

## Attempts / Log
- Created task: `phase0-03-triangle-render.md` (2026-02-06)

---

## Contacts
- If you want me to implement this task, say which owner to assign and whether to use CI-based pixel checks (automated) or manual verification only.
