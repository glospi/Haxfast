# Vulkan and graphics acceleration

## Honest expectation first

HaxBall is a 2D game that draws on canvas and runs inside Chromium. Enabling
Vulkan in that context does **not** transform performance: Chromium already uses the GPU
for composition and rasterization with its default backend (GL/EGL). Vulkan can
help in a few scenarios (composition with many blits, certain drivers on
Wayland) and can make things worse in others.

Rule: the option is exposed, it is **measured**, and if it does not win, it is not recommended.

## What can be done today (inside the Chromium client)

1. **Backend selector** (F2): `--use-angle=vulkan` + associated features, versus
   `--use-angle=gl`/default. Configurable from the UI, with a client restart.
2. **Verification HUD** (F2): show which backend actually ended up active.
   Chromium can silently fall back to software or GL: the selector without
   verification is self-deception.
3. **Measurement** (F2): frame times with the HUD in real matches (room with people,
   big stadium) to decide with data.

The detail of the exact flags is defined in F2 when the selector is implemented
and a real comparison is possible; nothing is hardcoded before that.

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
