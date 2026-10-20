# Performance and metrics (F2)

How HaxFast measures frame time, controls the frame-rate governor and selects the
GPU backend. Companion to [ENGINE.md](ENGINE.md) (patch pipeline) and
[DECISIONS.md](DECISIONS.md) (D21–D24).

## What it is

Three pieces, all fail-open:

1. A **frame gauger** in the engine runtime: counts the draws the game actually
   makes, keeps the last 256 frame intervals and reports FPS + p95.
2. A **governor** with three modes (`gsync`, `locked`, `unlimited`) that decides
   the Chromium startup flags and, for `locked`, a live draw cap.
3. A **HUD** in the top frame with the live metrics, the applied patch state, the
   real GPU backend and a settings panel persisted in `preferences.json`.

Simulation, network and input never slow down: the cap is applied inside the
game's live-view draw guard (D22).

## Governor modes

| Mode | Chromium startup flags | Draw cap | Applied |
| --- | --- | --- | --- |
| `gsync` (default) | none | none | startup (no flags) |
| `locked` | `--disable-frame-rate-limit` `--disable-gpu-vsync` | `fps_limit`, 10–1000, default 144 | flags at startup; cap live |
| `unlimited` | same two flags | none | startup |

- `gsync` is the balanced default: no flags, no cap, the same path the official
  client uses.
- `locked` and `unlimited` disable Chromium's own frame-rate limit and vsync. The
  difference is that `locked` asks the engine runtime to skip draws beyond the
  target; `unlimited` draws every frame the compositor delivers.
- Changing the mode or the GPU backend is a **startup** choice (Chromium flags):
  the HUD shows `restart to apply`. Changing the limit or a patch toggle is live.
- The game's own **FPS Limit** (`localStorage.fps_limit = 1` → 30 FPS) keeps
  working: its guard runs before the governor and the stricter cap wins. The HUD
  shows `native 30` and never changes that setting.

## Draw-only cap (why simulation is untouched)

The live loop is a single minified method; the F2 patch replaces its draw guard:

```diff
-Rc(){var a=window.performance.now();1==m.j.Qh.v()&&28.333333333333336>a-this.hd||(this.hd=a,this.Bd++,…)
+Rc(){var a=window.performance.now();__HAXFAST_ENGINE__.skipDraw(a,1==m.j.Qh.v()&&28.333333333333336>a-this.hd)||(this.hd=a,this.Bd++,…)
```

`skipDraw(now, nativeBlocked)` returns `true` when the draw must be skipped:
`nativeBlocked` (the vanilla FPS Limit) or when less than 90% of the cap interval
(`1000 / fps_limit`) has elapsed — the 10% early tolerance that keeps a 30 target
from alternating 20/30 on a 60 Hz loop (D22). Returning `false` lets the original
code update `this.hd` and `Bd` (the game's own counter). The rest of `Rc`
(simulation, network, input, scheduling) still runs at the monitor cadence.

Fail-open at every step: missing anchor → no replacement (vanilla file); toggle
off or any exception → `nativeBlocked` (exactly vanilla). The replay/menu loop
has a different anchor and is not patched; during a replay the HUD falls back to
the loop cadence until live draws resume.

## Metrics

- Source: engine runtime `frameStats()`, sampled by the HUD every 250 ms.
- Window: last 256 draw intervals; p95 by nearest rank; intervals ≥ 250 ms are
  discarded (tab resumes, breakpoints); the window resets when the page is hidden
  and when the cap changes. If no draw happened for 500 ms the runtime reports 0
  samples, so the HUD never freezes on stale values. 256 ≈ 4 s at 60 fps and
  ≈ 8.5 s at 30 fps; a 2048 window was measured first and rejected because a
  change to the game's native 30 limit took over a minute to show (D24).
- Fallback: if the engine runtime or the `frame-governor` patch is absent, the
  toggle is off, or there are no recent draws (menu, replay, room list), a rAF
  sampler inside the game frame measures loop cadence; the HUD marks it with `~`.
  It is a fallback, not the metric: it cannot see skipped draws.
- Log: every 5 s the HUD sends `metrics.frame` to the host (`fps`, `p95`,
  `samples`, `source`, `mode`, `limit`, `cap`, `gpu`, `patches`, `native30`),
  written to `logs/haxfast.log` as `[page] frame {…}`.

## HUD

- Top-frame overlay, bottom-right; visible at startup (`video.show_hud`). It lives
  in the top frame because the bridge and the future shell UI live there (F3); the
  game is touched only through its runtime object.
- Line: `fps · p95 · mode · applied patches · backend [· native 30]`.
- Click the line for the panel:
  - governor (restart note), FPS limit (live, 10–1000),
  - GPU backend (restart note) and the full WebGL renderer string,
  - engine patch checkboxes (live; an anchor missing this startup is disabled).
- `F8` hides/shows it and persists `video.show_hud`. **Provisional**: F5 replaces
  it with configurable binds. The listener is installed in both the top frame and
  the game iframe (key events do not cross frame boundaries) and is ignored while
  typing in a field.
- The HUD is fail-open too: if the page bundle cannot find the game frame, the
  game keeps playing without it.

## GPU backend selector

`video.gpu_backend`:

| Value | Flag | Notes |
| --- | --- | --- |
| `auto` (default) | none | Chromium's default (GL/EGL on Linux) |
| `gl` | `--use-angle=gl` | explicit ANGLE GL |
| `vulkan` | `--use-angle=vulkan` | ANGLE over Vulkan |

Startup only. The HUD shows the **actual** renderer from
`WEBGL_debug_renderer_info`; a silent fallback to GL or SwiftShader is visible
there. Honest expectations and the measured comparison live in [VULKAN.md](VULKAN.md).

## Persistence

`preferences.json`:

```json
{
  "engine": { "…": true, "frame_governor": true },
  "video": {
    "governor": "gsync",
    "fps_limit": 144,
    "show_hud": true,
    "gpu_backend": "auto"
  }
}
```

The host validates leniently: unknown `governor`/`gpu_backend` strings, a missing
`video` section or an out-of-range `fps_limit` fall back/clamp at read time and
never reset the profiles (D4/D21); the host logs the effective values at startup.
The HUD writes the whole object through `prefs.set`, the same bridge path D20
reserved for the engine toggles.

## Verification

```sh
cargo test --manifest-path native/Cargo.toml
npm run check:page && npm run build:page
HAXFAST_HOME=/tmp/opencode/haxfast-f2 HAXFAST_REMOTE_DEBUG=9222 \
  ./native/target/debug/haxfast-host
```

Logs at `/tmp/opencode/haxfast-f2/logs/haxfast.log`. Useful lines:

```
cef: startup flags (governor gsync, no cap, gpu auto)
cef: startup flags (governor locked, 144 fps, gpu gl)
cef: startup flags (governor unlimited, no cap, gpu vulkan)
engine: game-min.js ready (… patches: runtime, …, hidden-frame-skip, frame-governor)
[page] prefs received {"profiles":1,"governor":"gsync"}
[page] frame {"fps":60,"p95":17.1,"samples":256,"source":"engine",…,"native30":false,…}
```

Devtools/CDP:

```js
__HAXFAST_ENGINE__.frameStats()        // {fps, p95, samples, draws, limit}
__HAXFAST_ENGINE__.setFrameLimit(144)  // live cap; 0 removes it
window.__HF_SELFTEST__()               // metrics/clamp assertions, [] on success
```

`F8` toggles the HUD; the panel's changes go through `prefs.set` and are persisted
immediately. Mode and backend take effect on the next start; the limit and the
engine toggles apply live.

## Measured results

Session 19-10-2026. Machine: Arch Linux (CachyOS kernel 7.2.11), Intel Core
i3-N300, Intel Graphics (ADL-N), Mesa (OpenGL 4.6 / Vulkan 1.4.354), X11 at
60 Hz, CEF window 1280×720. Values are steady state from the engine draw counter
(`source: engine`), last 256 intervals, sampled by the 5 s log line; `auto`
resolves to GL here. Scenes: **boltball** = public `⚡️ boltball.io | Futsal 3v3 |
Ranked`, 0/16 with bots playing; **6.5** = `ㅤ⚡ 6.5 | Futsal x1「T3」`, 0/10 with a
live match. Both are live draw loops, not menus.

| Mode/backend | Scene | FPS | p95 (ms) | Actual renderer |
| --- | --- | --- | --- | --- |
| `gsync` (default, `auto`/`gl`) | boltball, bots | 60.0 | 17.1–17.9 | ANGLE (Intel, Mesa Intel(R) Graphics (ADL-N), OpenGL 4.6) |
| `locked 30` | 6.5, live | 32.5 | 31.5 | same GL |
| `locked 144` | 6.5, live | 140.0 | 7.9 | same GL |
| `unlimited` | 6.5, live | 331.8 | 4.0 | same GL |
| `gsync` + `gpu_backend=gl` | boltball, bots | 60.0 | 17.1 | ANGLE (Intel, Mesa Intel(R) Graphics (ADL-N), OpenGL 4.6) |
| `gsync` + `gpu_backend=vulkan` | boltball, bots | 47.1 | 26.2 | ANGLE (Intel, Vulkan 1.4.354 (Intel(R) Graphics (ADL-N) (0x000046D0)), Intel open-source Mesa driver) |
| `gsync` + game native 30 | boltball, bots | 30.0 | 34.6 | Vulkan (the native guard is backend-independent) |
| menu / room list (fallback) | — | ~60 | 17.9 | GL, rAF sampler (approximate, `~`) |

Notes:

- **The cap is a ceiling, not an exact rate.** The 10% early tolerance and the
  loop's granularity put `locked 30` at 32.5 fps (some 16.7 ms and some 33.3 ms
  intervals) and `locked 144` at 140.0 fps (the unlocked loop runs at ~330 Hz,
  and jitter occasionally costs an extra interval). Without the tolerance, 30 on
  a 60 Hz loop alternated 20/30. The average can exceed the target slightly; p95
  is the honest tail.
- **`unlimited` is scene-dependent**: 331.8 fps in the 0/10 room; earlier runs
  measured 292 fps in a different room. It is not a fixed number.
- **Vulkan works but loses here**: real ANV renderer, 47.1 fps and +9 ms p95
  against GL in the same room. The selector stays (other GPUs/drivers may
  differ); on this machine `auto`/`gl` is the recommendation ([VULKAN.md](VULKAN.md)).
- **Native 30 wins over the governor**: with the game's own `FPS Limit = 30`,
  actual draws are 30.0 fps and the HUD appends `native 30`; the stricter cap is
  the one that matters. The F2 runtime never changes that game setting.
- A 6-player public room measured 60.0 fps / p95 17.7 ms (gsync, GL, 2048-window
  run before the 256 change); the draw path is the same as the bot scene.
- The menu/room-list row is the rAF fallback: the live loop does not draw there,
  so it is loop cadence marked `~`, not a draw metric.

## Files

- `native/crates/core/src/prefs.rs` — `VideoPrefs`, modes, backend, clamping.
- `native/crates/host/src/app.rs` — startup switches (governor, ANGLE).
- `native/crates/host/src/engine.rs` + `runtime.js` — `frame-governor` patch,
  skipDraw, frameStats.
- `native/crates/host/src/bridge.rs` — `metrics.frame` log sink.
- `page/src/index.ts`, `hud.ts`, `metrics.ts`, `governor.ts`, `game.ts`,
  `types.ts` — HUD, sampling, governor control and persistence.
