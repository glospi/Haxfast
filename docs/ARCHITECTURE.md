# Architecture

## Overview

HaxFast does not reimplement HaxBall. It embeds Chromium (CEF) to run the
official engine (`game-min.js`) exactly as the site serves it, and adds everything else
around it:

```
┌─────────────────────────────────────────────────────────────────┐
│ haxfast-host (Rust, main CEF process)                           │
│                                                                 │
│  paths · log · bridge ──────┐        engine.rs: downloads and   │
│  (local WS 127.0.0.1+token) │        patches game-min.js        │
│                             │        (fail-open)                │
│  app.rs: window (Views),    │                                   │
│  handlers, injection ───────┤                                   │
└─────────────────────────────┼───────────────────────────────────┘
                              │
        ┌─────────────────────┴─────────────────────┐
        │ CEF / Chromium (renderers, GPU, net)      │
        │  · loads https://www.haxball.com/play     │
        │  · game-min.js request → served by HaxFast│
        │  · runs the injected dist/page.js         │
        └─────────────────────┬─────────────────────┘
                              │ local WebSocket (JSON)
                    ┌─────────┴─────────┐
                    │ page/ (TS → JS)   │  bridge, HUD, skins, shortcuts…
                    └───────────────────┘
```

And the third component, meant to grow until it becomes the native client:

```
native/crates/core   haxfast-core: pure logic. No CEF, no DOM, no network of its own.
                     Preferences, validations, text patches and, in the future, the
                     simulation. It is the seed of the native track (NATIVE-TRACK.md).
```

## The three boundaries

### 1. Host → page: script injection

When the main frame finishes loading (`LoadHandler::on_load_end`), the host
runs:

1. `window.__HAXFAST_BOOT = { wsUrl, token }` — how to reach the bridge.
2. `dist/page.js` — the page layer bundle.

It is re-read from disk on every load: rebuild + reload is enough as a
development cycle. The script runs in the page's main world (no isolated
world) because it needs to see exactly what the game sees.

### 2. Page → host: WebSocket bridge

`page/src/bridge.ts` is the **only** point of contact. The host starts a
WebSocket on `127.0.0.1` with an ephemeral port and a random token; the token
travels in the first message (`hello`). Current methods: `prefs.get`, `prefs.set`,
`log`, `metrics.frame`.

Why WebSocket and not CEF's native IPC (`cefQuery`/MessageRouter): the
MessageRouter requires handlers in the browser and renderer processes, size limits and
its own format. The WebSocket is simpler, debuggable from plain devtools, and the
decision is **reversible without touching anything else**: change `bridge.ts` and the
`bridge.rs` module.

### 3. Host → engine: interception of `game-min.js`

CEF's `RequestHandler` sees every request. When the URL is
`*/game-min.js`, it is answered from memory with the already patched bundle
(`engine.rs`) instead of letting the request go through to the network.

- The original download happens **once at startup** (blocking, with timeout):
  deterministic, with no race conditions against the page load.
- The patches are **fail-open**: if the download fails, if the anchor does not
  appear or if any step breaks, the intact official file is served and it is
  logged. The project rule is that nothing in HaxFast may prevent entering and
  playing.
- The "runtime" (`__HAXFAST_ENGINE__`) is inserted **after** `'use strict';`
  so as not to take the file out of strict mode.

The same `RequestHandler` cancels known ad/sync hosts before they reach the
network (`ads.rs`); the page layer also hides the official `div.rightbar`. Both
are fail-open. The only browser UI kept is a Copy/Paste context menu; the F3
shell replaces the official header and decides external-link behavior.

## Performance rules

The project's goal is speed, but speed is not achieved by rewriting
everything: it is achieved by not moving more data than necessary.

- **Hot paths live where they already are**: HUD, FPS governor, skins and everything
  that depends on `requestAnimationFrame`/DOM live in JS, inside the page, glued
  to the frame. Moving them to Rust adds a boundary crossing per frame and loses.
- **Logic and state live in Rust** when the cost is per event, not per frame:
  preferences, validation, download/patching, administration features.
- **DOD (data-oriented design) only where it hurts**: contiguous structures, `Vec`
  of components, indices instead of pointers, when there are hot loops of our
  own (native track simulation). Before that, normal readable code.
- Communication between layers is JSON over WebSocket. If one day a message is
  too hot, it gets its own binary path **without** changing the layer
  structure.

## Frame pacing and GPU backend (F2)

- **Governor**: `video.governor` is a startup choice (D21). `gsync` leaves
  Chromium's flags untouched; `locked`/`unlimited` add
  `--disable-frame-rate-limit` + `--disable-gpu-vsync`. The `locked` cap is applied
  live by the engine patch in the live-view draw guard, so simulation, network and
  input keep the monitor cadence (D22).
- **Metrics**: the engine runtime counts the draws it actually makes; the page HUD
  samples `frameStats()` and persists `metrics.frame` every 5 s (D24). A rAF
  sampler is only the fail-open fallback, marked `~`.
- **GPU backend**: `video.gpu_backend` maps to `--use-angle=gl|vulkan` (auto = no
  flag), also startup-only. The HUD shows the real WebGL renderer string so a
  silent fallback is visible; measured results in [PERFORMANCE.md](PERFORMANCE.md)
  and [VULKAN.md](VULKAN.md).
- The HUD, the governor and the sampling live in the top frame (`page/`), the
  draw counter and the cap inside the engine runtime. All changes persist through
  `prefs.set`; a missing anchor or a dead runtime never blocks play.

## Reserved for the future

- **Vulkan acceleration**: today the game is 2D on canvas and runs inside
  Chromium; F2 exposes the ANGLE backend choice and measures it, but a render of
  our own with `wgpu` remains reserved as `native/crates/render/` (it is not
  created until there is something to draw). See [VULKAN.md](VULKAN.md).
- **Full native client**: the core is kept render-agnostic precisely
  so that the day a client without Chromium exists, the core does not change. See
  [NATIVE-TRACK.md](NATIVE-TRACK.md).
- **wasm adapter**: if the page needs core logic (e.g. shortcut
  economy calculation), a `wasm` crate with `wasm-bindgen` is born in F5. It is not
  created before that: today nobody consumes it.
