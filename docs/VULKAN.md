# Vulkan and graphics acceleration

## Honest expectation first

HaxBall is a 2D game that draws on canvas and runs inside Chromium. Enabling
Vulkan in that context does **not** transform performance: Chromium already uses the GPU
for composition and rasterization with its default backend (GL/EGL). Vulkan can
help in a few scenarios (composition with many blits, certain drivers on
Wayland) and can make things worse in others.

Rule: the option is exposed, it is **measured**, and if it does not win, it is not recommended.

## What is implemented today (F2)

1. **Backend selector**: `video.gpu_backend` = `auto` | `gl` | `vulkan`; `gl`/`vulkan`
   add `--use-angle=gl` / `--use-angle=vulkan`, `auto` adds nothing. Startup only;
   the HUD shows a restart note when it changes.
2. **Verification HUD**: the HUD line and panel show the **actual** renderer from
   `WEBGL_debug_renderer_info`, so a silent fallback to GL or SwiftShader is visible.
3. **Measurement**: real draw intervals from the engine runtime (not rAF), sampled
   every 250 ms and logged every 5 s. The method and scene table live in
   [PERFORMANCE.md](PERFORMANCE.md).

## Measured on the F2 machine

Arch Linux, Intel Core i3-N300 / Intel Graphics (ADL-N), Mesa OpenGL 4.6 and
Vulkan 1.4.354, X11 at 60 Hz, same room (`boltball.io | Futsal 3v3`, bots), gsync,
last 256 draw intervals:

| Backend | FPS | p95 (ms) | Renderer |
| --- | --- | --- | --- |
| `auto` (GL here) | 60.0 | 17.1 | ANGLE (Intel, Mesa Intel(R) Graphics (ADL-N), OpenGL 4.6) |
| `gl` | 60.0 | 17.1 | same |
| `vulkan` | 47.1 | 26.2 | ANGLE (Intel, Vulkan 1.4.354 (Intel(R) Graphics (ADL-N) (0x000046D0)), Intel open-source Mesa driver) |

Vulkan is real (Mesa ANV, not SwiftShader) but **loses** ~13 fps and adds ~9 ms to
p95 here. It stays available because other GPUs/drivers may differ; on this
machine `auto`/`gl` is the recommendation. The selector is not a promise of
improvement (see above).

| Value | Flag |
| --- | --- |
| `auto` (default) | none |
| `gl` | `--use-angle=gl` |
| `vulkan` | `--use-angle=vulkan` |

## Space reserved for the future

If some day HaxFast renders on its own (native track, see NATIVE-TRACK.md):

- The reserved crate is `native/crates/render/` using `wgpu` (which speaks Vulkan,
  Metal or D3D12 depending on the platform). **It is not created until there is something to draw.**
- The `core` knows nothing about graphics or Vulkan: it produces state (positions, radii,
  chat text) and the render only draws it. This boundary is what allows the core
  not to change a single line on the day of the native render.
- Realistic objective of the native render: deterministic input latency and frame time,
  not "more FPS" in the abstract.

## What is not going to be done

- Replacing the canvas of the official game with a render of our own while
  `game-min.js` is used: the official engine draws on its own and hacking its canvas is more
  fragile than useful.
- Presenting Vulkan as a guaranteed improvement in the documentation or the UI.
