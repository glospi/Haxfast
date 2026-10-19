# Engine (F1)

How the bundle served to the browser differs from the official `game-min.js`, and
how to control it. Reference for the patch-by-patch behavior: it is the document
the verification recipe points to when a patch is in doubt.

## What it is

`crates/host/src/engine.rs` downloads/resolves the official bundle and applies a
small number of **text substitutions** before serving it. Each patch has one unique
anchor verified against the live bundle, one replacement, and one toggle. The rules
(D5, unchanged):

- **Fail-open**: a missing anchor skips only its patch (logged as WARN); a runtime
  error falls back to the vanilla rendering for that call; nothing ever stops the
  game from playing.
- No version matrices and no fingerprints: either the anchor is there this startup
  or the patch is left out (a later deploy or the disk cache re-evaluates it).
- One anchor per patch; the anchors were verified unique against the live bundle
  (173 175 B, sha256 `322d1904…885`, deploy `H4mQWPXF`).

## Runtime

The patched bundle starts with a small runtime (`crates/host/src/runtime.js`,
injected right after `'use strict'`; the file never leaves strict mode):

```
globalThis.__HAXFAST_ENGINE__ = {
  version, patches: [...applied names...], toggles: { 'canvas-resize-cache': true, ... },
  on(name), set(name, value), rect(canvas), stadium(T, zoom, stadium), shouldRender()
}
```

Every patch routes through `on()`, read **live**: `set('stadium-layer-cache', false)`
from the console takes effect the next frame, no reload. The toggles are seeded
from `preferences.json` (section `engine`, all default ON) at startup; changing the
file needs a restart, `set()` does not survive it (UI and persistence of changes
arrive in F2).

Useful from devtools or CDP:

```js
__HAXFAST_ENGINE__.patches          // what got applied this startup
__HAXFAST_ENGINE__.set('hidden-frame-skip', false)
```

## Patches

### zero-input-tolerance

- Anchor: `this.Od=2` → `this.Od=0` (the vanilla value 2 is the input tolerance in
  60 Hz ticks, ~33 ms of dead time; 0 means immediate response). The following
  `this.vo=600;super.Za(` (vanilla serialization) stays untouched.
- Toggle off → the vanilla `2` is served.

### canvas-resize-cache

- Anchor: `this.oa.getBoundingClientRect()` → `__HAXFAST_ENGINE__.rect(this.oa)`.
- Why only this: the vanilla resize method already guards the backing-store write
  (`if (this.oa.width != c || this.oa.height != a)`), so the per-frame cost left is
  the layout read. `rect()` caches it per canvas with a `ResizeObserver`; the
  `devicePixelRatio` multiply stays live at the call site.

### stadium-layer-cache

- Anchor: `this.Dr(c.U);this.Cr(c.U);` → `__HAXFAST_ENGINE__.stadium(this,h,c.U);`
  (in the renderer `T.Rc`, where the world transform `scale(h) translate(-Ya)` is
  already active and `h` is the exact zoom; `e.H` only reassigned `h` afterwards).
- What it does: bakes `Dr` (background + pitch) and `Cr` (layers) — the static
  stadium, drawn every vanilla frame — into an offscreen texture covering a
  **2× viewport world region** around the camera, snapped to a 64-world-unit grid.
  Inside the central band of that region a frame is a single `drawImage` under the
  already-active transform (pixel-perfect: texture scale equals the zoom).
- Re-bakes when: the visible rect leaves the central half, the zoom changes, or the
  canvas backing size changes.
- Mechanics worth knowing: `Dr` type 1 fills the full canvas in screen pixels after
  `resetTransform`, so during a bake `T.oa` is shadowed by `{width, height}` of the
  texture; the pitch patterns (`tp/Fo/Do`) are created on the main context and used
  cross-context on the bake context; any exception restores the real canvas
  immediately and draws the frame vanilla.
- shortcut: over the ~16 MP texture budget (≈ 4K fullscreen) the texture is scaled
  down and slightly blurred instead of skipped; if that ever bothers a retina user,
  raise the budget or skip the patch (upgrade: per-region tiles).
- shortcut: `ya` (crowd) is *not* cached — it animates; `wr/yr/Ar` stay per frame.

### hidden-frame-skip

- Anchor: `this.Re=window.requestAnimationFrame(M(this,this.sf));this.W.A();` — the
  `this.W.A()` part makes it unique: the needle appends
  `if(__HAXFAST_ENGINE__.shouldRender())return;` after the rAF scheduling of the
  live-view loop, so while the page is hidden the whole per-frame chain stops.
- The second loop class (replay/menu, same rAF expression but a different tail) is
  untouched. Resuming costs one long frame delta — exactly the vanilla behavior when
  a tab is hidden, so nothing new. `shouldRender()` = toggle on AND
  `document.visibilityState === "visible"`.

## Disk cache of the original bundle

`$HAXFAST_HOME/engine/` (never the patched bytes): every deploy gets two files,
`<id>.js` (original bundle) and `<id>.json` (`{url, hash}`); the id is a hash of the
bundle URL and the hash is a content digest (std `DefaultHasher`, no dependencies).
The cache is only a hit when both the URL and the re-computed content hash match;
anything else (corrupt file, different deploy, deleted meta) falls back to the
network. Deleting the folder is always safe. Deploy rotation produces one extra
pair of files per deploy.

## Where the knobs live

| What | Where |
| --- | --- |
| Patch toggles (persisted) | `preferences.json` → `engine.{canvas_resize_cache, stadium_layer_cache, zero_input_tolerance, hidden_frame_skip}` (snake_case; old files default to all ON) |
| Patch toggles (live, not persisted) | `__HAXFAST_ENGINE__.set(...)` |
| Skip a patch forever | its anchor is gone (fail-open), or set its toggle |

## Verification

`cargo test --manifest-path native/Cargo.toml` covers the pipeline (anchors,
fail-open, toggles off, cache round-trip and corruption). The live recipe is in
`AGENTS.md` §6; the F1 session (19-10-2026) checked:

- first start: `engine: game-min.js ready (173175 bytes (download), patches: runtime,
  zero-input-tolerance, canvas-resize-cache, stadium-layer-cache, hidden-frame-skip)`,
  interception, `__HAXFAST_ENGINE__` alive inside the game iframe (version, patches,
  toggles), `set()` A/B round-trip true → false → true;
- second start: the same line but `(disk cache)` — no bundle download;
- no JS exceptions or console errors during normal play with all four patches on;
  scheduling of the rAF loop continues while hidden (only the draw chain stops).
- live sessions also exercised the two bugs fixed before closing F1: the inverted
  hidden-frame guard (every frame returned before resize/draw) and the stadium bake
  reuse window, which could leave up to 280 px of the left edge with the previous
  frame after panning left. The bake now blits only while the transformed visible
  rect (HUD offset included) is fully inside the baked region.
