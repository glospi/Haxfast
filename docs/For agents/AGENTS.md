# AGENTS.md — guide for agents continuing HaxFast

This file is the operational entry point. The project truth lives in
[../README.md](../README.md), [../ARCHITECTURE.md](../ARCHITECTURE.md),
[../DECISIONS.md](../DECISIONS.md) (whys and traps) and [../ROADMAP.md](../ROADMAP.md)
(phases). Here goes only what an agent needs to work without breaking anything.

## 1. What it is and hard rules

High-performance HaxBall client: Rust host with embedded Chromium (CEF) +
TypeScript page layer injected over the official site + pure `haxfast-core`
(seed of the future native client).

Rules that are not negotiable:

- **Nothing may prevent entering and playing.** Every patch is fail-open: if something
  fails, the intact official `game-min.js` is served and logged. Never break to "improve".
- **Speed first**, but without rewriting for fun: don't move more data than needed.
  Hot paths (per frame) in JS inside the page; logic and state (per event) in Rust.
- **The frame cap is draw-only**: never slow simulation, network or input to hit an
  FPS target (D22).
- **In v1 Rust does not intercept WebRTC, signaling or the protocol.** That is touched
  only in the native track (R).
- One **anchor per patch**: if the text is not there, that patch is not applied (there
  are no version matrices or fingerprints).
- **Code, comments and docs in English; comments explain the *why*, not the *what*.**
- **Readable and modular** code over cleverness. Ask the user
  when there is doubt about product or scope; don't decide silently.
- GPL-3.0-only, public repo at github.com/glospi/Haxfast, no CI yet; Linux first
  (Windows/macOS documented, not maintained).
- Don't vendor the All-in-One extension or `game-min.js`; our own features are ported.
- Don't add dependencies without discussing it.

## 2. Current state (19-10-2026)

**F0 closed on 18-10-2026** — all 10 items green, documented in
[../GATE-F0.md](../GATE-F0.md):

- Items 1–6 ✔: host starts with a single window (ALLOY, no Chrome UI), loads
  `https://www.haxball.com/play`, serves the patched `game-min.js`, the game reaches the
  main menu, the bridge responds (`prefs.set`/`prefs.get`) and the frame-rate flags
  reach CEF (F0 added them unconditionally; F2 made it a governor mode, D21).
- Items 7–10 ✔ (manual, real room, confirmed by the user): joining and playing,
  reCAPTCHA, audio and fullscreen. The logs of that session show no WebRTC/STUN or
  console errors.
- **F1 closed on 19-10-2026** — [../ENGINE.md](../ENGINE.md): four fail-open patches
  over `game-min.js` (input tolerance 0, cached canvas resize, cached stadium
  layers, hidden-frame skip), the `__HAXFAST_ENGINE__` runtime with live toggles and
  the original-bundle disk cache. Two live-found bugs fixed before closing (inverted
  hidden-frame guard, stadium bake coverage).
- **F2 closed on 19-10-2026** — [../PERFORMANCE.md](../PERFORMANCE.md): persisted
  governor (`gsync` default, `locked` with a live draw cap, `unlimited`), HUD visible
  at startup (`F8`, settings panel), real draw metrics + p95 with a marked rAF
  fallback, ANGLE backend selector showing the actual renderer, measured GL/Vulkan.
  The engine patch is `frame-governor` ([../ENGINE.md](../ENGINE.md)); D21–D24.
  Honest limits: the cap is a ceiling (10% tolerance), Vulkan was slower on the test
  machine, the menu/replay loop is not patched, `F8` is provisional until F5.
- **Current phase: F3** (shell UI and skins), not started.

## 3. Repo map

```
native/crates/core     haxfast-core: pure logic (prefs, validation). No CEF or DOM.
native/crates/host     CEF binary: main (startup/preload), app (handlers, window,
                       injection), engine (bundle download, patching, disk cache),
                       runtime.js (injected engine runtime), bridge (WS), paths, log.
page/src               Page layer (TS → dist/page.js): bridge.ts (only channel to the
                       host), index.ts (bootstrap + tick), hud.ts (overlay, panel,
                       F8), metrics.ts (rAF fallback + selftest), governor.ts (apply/
                       persist caps), game.ts (iframe access), types.ts (shared).
build/build.mjs        Page build (esbuild, --watch in dev).
docs/                  Normative documentation.
docs/reference-me/        REA reports: historical material, local only (git-ignored).
resources/skins        Empty until F3.
assets/ test/          Empty for now.
```

## 4. Build, run, test

```sh
npm install
npm run build:page                 # dist/page.js (re-read on every load: rebuild + F5 is enough)
cargo build --manifest-path native/Cargo.toml -p haxfast-host
./native/target/debug/haxfast-host
cargo test --manifest-path native/Cargo.toml   # core (9) + host (13)
npm run check:page                             # tsc --noEmit
```

The first host build downloads the CEF binaries (~200 MB) and compiles the
wrapper: it can take minutes.

## 5. Environment variables

| Variable | Effect |
| --- | --- |
| `HAXFAST_HOME` | Data folder (default `~/.config/haxfast`) |
| `HAXFAST_URL` | Page to load (default `https://www.haxball.com/play`) |
| `HAXFAST_REMOTE_DEBUG=9222` | Chromium CDP port (loopback) |
| `HAXFAST_PAGE_BUNDLE` | Alternative path to `dist/page.js` (if missing, it does not inject) |
| `HAXFAST_PRELOADED=1` | Internal use: marks the re-exec already done with `LD_PRELOAD` |

## 6. Verification (recipe)

To test a change without dragging along cache or old prefs:

```sh
HAXFAST_HOME=/tmp/opencode/haxfast-fresh HAXFAST_REMOTE_DEBUG=9222 ./native/target/debug/haxfast-host &
tail -f /tmp/opencode/haxfast-fresh/logs/haxfast.log
```

Log signals (all must appear):

```
cef: startup flags (governor gsync, no cap, gpu auto)
engine: game-min.js ready (... bytes (download|disk cache), patches: runtime, zero-input-tolerance, canvas-resize-cache, stadium-layer-cache, hidden-frame-skip, frame-governor)
engine: intercepted https://.../game-min.js
page: loaded https://www.haxball.com/play (HTTP 200)
page: bundle injected (dist/page.js)
[page] page bootstrap / prefs received / frame {"fps":...,"p95":...,"samples":...,"source":"engine",...}
```

A patch missing its anchor logs `engine: <name> skipped: anchor not found (fail-open)`
— the game keeps working, only that patch stays out.

With CDP (`http://127.0.0.1:9222/json/list` → target WebSocket), check **inside the game
iframe** (the bridge and the engine live in different frames):

```js
const f = [...document.querySelectorAll("iframe")].find((i) => (i.src || "").includes("game.html"));
JSON.stringify({
  gameFrame: f?.src,
  engine: f?.contentWindow.__HAXFAST_ENGINE__,     // { version, patches } = patched bundle running
  frames: f?.contentWindow.__HAXFAST_ENGINE__?.frameStats?.(), // real draws: {fps,p95,samples,draws,limit}
  dialog: f?.contentDocument.querySelector(".dialog h1")?.textContent, // "Choose nickname" = it started
  bridge: await window.__HF_BRIDGE__.call("prefs.get"), // the host answers
  hud: document.querySelector("#haxfast-hud")?.children[0]?.textContent, // line + "~" fallback
  selftest: window.__HF_SELFTEST__?.(),            // [] = metrics/clamp checks pass
});
```

The HUD starts visible and `F8` hides/shows it (`video.show_hud` is persisted).
The line starts with `~` when it is the rAF fallback (menu, replay, room list,
where the live loop is not drawing); without it, the numbers come from actual
draws. A pending mode/backend change shows `restart to apply` in the panel.
`metrics.frame` is the bridge method (not `metrics.fps`).

And to see the bytes actually served (they must carry `__HAXFAST_ENGINE__` and the
patch sites, e.g. `__HAXFAST_ENGINE__.stadium(this,h,c.U)`):
`await fetch(f.src.replace(/game\.html$/, "game-min.js")).then(r => r.text())`.

A screenshot (`Page.captureScreenshot`) is the fastest visual proof that the game
started. Write the test scripts in `/tmp/opencode` (ephemeral) or ask the user whether
they are worth versioning.

## 7. Traps already paid for (don't step on them again)

- **`on_before_resource_load` must return an explicit `ReturnValue::CONTINUE`.** The
  cef-rs default (`ReturnValue::default()` and the trait's) is **`RV_CANCEL`**:
  CEF cancels the request and **never** calls `GetResourceHandler`, so the patch is
  not served and the page loses the resource. See DECISIONS D15.
- **Navigate to `/play`, never to the resolved `game.html`**: as the main document it
  stays in "Loading" (it waits for its parent page). The game runs in the `gameframe`
  iframe (same origin). See DECISIONS D14.
- **The game URL prefix changes on every deploy** (`/H4mQWPXF/__cache_static__/g/…`):
  it is resolved by parsing the `/play` HTML; never hardcode it.
- **The crash of libcef's `close` interposer** is avoided with a re-exec of our own in
  `main.rs` (`LD_PRELOAD=libcef.so` with a bare soname + `LD_LIBRARY_PATH=$ORIGIN`,
  flag `HAXFAST_PRELOADED=1`). The `DT_NEEDED` order cannot be fixed from `build.rs`;
  the `$ORIGIN` rpath can.
- **Chromium ≥ 138 blocks the local network**: without
  `ContentSettingTypes::LOOPBACK_NETWORK = ALLOW` in the global context, the page
  cannot open the bridge's `ws://127.0.0.1` WebSocket.
- **`RuntimeStyle::ALLOY`** in app and delegates: with the other style Chrome UI/tabs
  and extra windows show up.
- `--no-zygote` was a false culprit: don't re-add it "just in case".
- To kill old instances: by **process group** (`kill -9 -$(ps -o pgid= -p PID)`).
  **Never `pkill -f haxfast-host`**: it kills the agent's own shell. Watch out for
  other client processes of the user: don't touch them.
- The measured frame rate depends on the environment: with the monitor active and
  `gsync`, the compositor caps at ~60 fps regardless of the client. `unlimited` in
  the same machine measured 292–332 fps, scene-dependent; never compare numbers from
  a menu/room list (the live loop is not drawing and the HUD shows the rAF fallback
  `~`).
- **The game's own FPS Limit wins.** If `localStorage.fps_limit` is `1` (game menu →
  Settings → Video → FPS Limit = 30), the engine skips draws before the governor; the
  HUD appends `native 30` and F2 never writes that setting. Turn it to “None” before
  measuring a governor mode, and apply changes from the game's settings UI (a stored
  value alone is not read until the game reloads).
- **A top-page reload can be blocked by the game iframe's `beforeunload`**, leaving
  the old game document (and its old settings) alive behind a fresh top page. The page
  layer re-detects the iframe by `contentDocument` (the WindowProxy survives
  navigations) and reattaches the sampler/hotkey; for tests, change the game setting
  in its UI, or clear `contentWindow.onbeforeunload` before reloading the iframe.
- **The cap is a ceiling.** The 10% early tolerance is deliberate (D22); a 30 target
  measures ~32.5 fps and 144 measures ~140, not exact values. Don't "fix" that by
  removing the tolerance: without it a 30 target alternating 20/30 is worse.

## 8. Work cycle

- Commits in English, with a phase prefix: `F1: <what>`; message that explains
  the why of the change, not a listing of files.
- Before taking something as done: `cargo test` and `npm run check:page` green, and the
  recipe in section 6 showing that the game still starts.
- Every new decision (or reversal) is written down in `docs/DECISIONS.md` with the
  existing format: decision, why, alternatives, consequence.
- Scope or product changes are discussed with the user before coding.
- Finishing a phase's work includes its document (e.g. `docs/ENGINE.md` in F1,
  `docs/PERFORMANCE.md` in F2).
- Braces: the page layer (TS/JS) is Allman, kept by hand; there is no formatter there.
  Rust keeps the default rustfmt style (D18): the Allman option is nightly-only and
  covers item braces only, so it was discarded — never hand-move Rust braces into
  Allman, the next `cargo fmt` would undo it.

## 9. Next step

**F3 — shell UI and skins** (details and exit criterion in
[../ROADMAP.md](../ROADMAP.md)): mount our own UI in the top frame (the bridge and
the HUD already live there), CSS-only skins, external links to the system browser,
and the ads/official-navbar decisions. F2 is closed: read
[../PERFORMANCE.md](../PERFORMANCE.md) and D21–D24 before touching the HUD or the
governor. Before writing engine anchors, verify each one against the real downloaded
bundle (not against memory or an old deploy).

## 10. Per-phase dossier

Rule: **do not specify distant phases**. Each phase is designed when it starts, with the
live bundle in front and the questions answered by the user. What is here is only what
is already decided, the material that exists and what to ask. The full exit criterion
is in [../ROADMAP.md](../ROADMAP.md) — it is not duplicated.

Note on references: `haxball client/haxball-client/` (the user's Electron client,
reference code), the All-in-One extension and the REA reports in `docs/reference-me/`
live **outside** the published repo (or are git-ignored) and may not exist on another
machine; if they are missing, ask the user for them. The extension is not vendored
(D9): ideas are read, our own modules are ported.

### F1 — Engine performance patches *(closed 19-10-2026)*

- Shipped: `runtime.js` (toggles + `rect`/`stadium`/`shouldRender`) injected after
  `'use strict'`; four text transforms in `engine.rs` with all anchors verified
  unique against the live bundle (see [../ENGINE.md](../ENGINE.md) for each one and
  its honest limits); `EngineToggles` in `core/prefs.rs` (`engine` section, all ON
  by default, D20); disk cache of the original bundle in `engine_dir()` (D19). The
  F0 marker patch is gone.
- Material: `reference-me/REA-pure-client-report.md §2` and `§4` (source of the
  anchors), `reference-me/REA-haxball-report.md` (renderer `T`, loop classes,
  `m.j` settings), `engine.rs` tests (fail-open cases are unit-tested, never
  against the live bundle).
- Known risks for F2+: the stadium bake is the only patch with real graphics logic —
  if a future stadium type ever renders wrong, the toggle `stadium-layer-cache`
  isolates it. `set()` changes are persisted by the F2 HUD (D23).

### F2 — Speed and metrics *(closed 19-10-2026)*

- Shipped: `video` prefs (`governor`, `fps_limit`, `show_hud`, `gpu_backend`) with
  lenient parsing; startup flags in `app.rs`; the `frame-governor` text patch in
  `engine.rs` (live-view draw guard) and the runtime's `setFrameLimit`/`skipDraw`/
  `frameStats`; the top-frame HUD/panel/hotkey and the governor control in `page/`;
  `metrics.frame` in the bridge; [../PERFORMANCE.md](../PERFORMANCE.md) with design,
  verification and measured results. Decisions D21–D24.
- Answers: default `gsync`; HUD visible at startup; mode and backend need a restart;
  `locked` default 144 (10–1000); the game's native 30 wins; F8 is provisional.
- Material: `reference-me/REA-pure-client-report.md §3` (graphics backend, FPS/video,
  minimum latency mode, canvas options), `VULKAN.md`, the live bundle guard.
- Known limits: the cap is a ceiling (10% tolerance, measured 32.5 for 30 and 140 for
  144); Vulkan was slower than GL on the test machine; the menu/replay loop is not
  patched (HUD `~`); F8 becomes configurable in F5. There is no TS test framework:
  `npm run check:page`, `__HF_SELFTEST__`, `frameStats()` and CDP are the checks.

### F3 — Shell UI and skins

- Already decided: CSS-only skins with no executable JS, folder/zip with `skin.json` +
  CSS + assets (D13); external links to the system browser; no launcher window; the
  window has no Chrome UI (ALLOY).
- Material: D13; `resources/skins`; the ad banners on `/play` are visible inside the
  window (note in GATE-F0).
- Questions: where our own menu lives (bar over the web vs overlay); whether the
  default skin hides the ads and/or the official navbar.
- First move: mount the UI in the top frame (the bridge already lives there) and touch
  the iframe through `contentDocument` only for game-specific things.

### F4 — Profiles and authentication

- Already decided: the profile shape already exists in `core::prefs` (`player_name`,
  `avatar`, `extrapolation`, `player_auth_key`, `geo_override`, `fav_rooms`) because the
  game reads that data from `localStorage`; atomic validation and save (D4).
- Material: `reference-me/REA-haxball-report.md §4` (identity and storage),
  `reference-me/REA-pure-client-report.md §5` (avatars) and `§6` (auth/antifake); in the
  reference client, `inject/src/{profiles,auth}.ts`.
- Questions: how the auth key is stored (the reference client stores it as is); whether
  profiles are imported from another client; local avatar or by URL.
- First move: confirm **which exact keys** the game reads from `localStorage` in the
  current deploy (live bundle) before writing the hot identity change.

### F5 — Shortcuts

- Already decided: key → command or message; the `wasm` crate is born only if the page
  needs core logic (D12); if not needed, it is pure JS.
- Material: in the reference client, `inject/src/shortcuts.ts` and `constants.ts`
  (command bar with `/e`, `/a` by default).
- Questions: default shortcut set; whether keys that simulate game input are also
  wanted (more fragile: depends on the bundle).
- First move: implement per event in the page (not per frame) and leave in Rust only
  what depends on prefs.

### F6 — Sounds

- Already decided: same interception pipeline as `game-min.js`, validation by magic
  bytes (`RIFF/WAVE`, `OggS`, `ID3`/MPEG, `fLaC`), ~5 MB per file, in
  `userData/sounds/`; changes apply on reload because reloading live kicks you out of
  the room (D13).
- Material: D13; the 7 sounds are `chat`, `highlight`, `kick`, `goal`, `join`, `leave`,
  `crowd` (ROADMAP F6).
- Questions: default volume and whether it is persisted per profile.
- First move: find the real sound URLs in the current deploy (bundle and CSS) before
  promising the interception replacement.

### F7 — Rooms and favorites

- Already decided: favorites are per profile (`fav_rooms` already exists in
  `core::prefs`) and there is auto-join on startup; the reference client has the same
  feature.
- Material: `reference-me/REA-haxball-report.md §3` (network) and `inject/src/favrooms.ts`.
- Questions: whether "live presence" is a requirement (periodic query to some endpoint)
  or static favorites with auto-join is enough; where the list is shown.
- First move: read §3 of the REA report and decide where presence comes from (HTTP or
  the `p2p.haxball.com` socket?). If there is no data, ask before inventing a backend.

### F8 — Game view skin and administration

- Already decided: the game view is the iframe (same origin, D14), so the skin is
  applied there through CSS/DOM; administration modules are ported, the extension is
  not vendored (D9).
- Material: in the reference client, `inject/src/{gameview,handleview}.ts`.
- Questions: scope of the admin panel (simple kick/ban or list with actions); whether
  the HUD replaces or only decorates the official chat.
- First move: map the iframe's `data-hook="…"` hooks in the current deploy (they change
  with the game) before hooking UI.

### F9 — Packaging

- Already decided: Linux only (D10), AppImage (and `.deb`) `haxfast-*.AppImage`, data in
  `~/.config/haxfast`; the binary already carries RUNPATH `$ORIGIN` and `libcef.so`
  travels next to it (`crates/host/build.rs`) — the AppImage is "copy the tree and wrap
  it".
- Questions: artifact signing; automatic or manual updates.
- First move: measure the final weight (CEF + resources), decide what is excluded and
  **re-evaluate the sandbox** (D2) before accepting the package.

### Track R — Native client (parallel from F2)

- Already decided: `core` stays render-agnostic; `crates/render` is not born until
  there is something to draw (ARCHITECTURE §Reserved for the future).
- Material: `NATIVE-TRACK.md`, `reference-me/REA-haxball-report.md §3` (signaling
  `wss://p2p.haxball.com`), `reference/estructuras-y-patrones-optimizacion.md` (for our
  own simulation).
- First move: REA capture of the room handshake and signaling CLI (steps 1–2 of the
  track), without touching the client that works today.

### Efficiency rule

Every phase starts with: (1) the exit criterion re-read, (2) the live bundle downloaded
and the selectors/anchors verified **against that text**, (3) the questions above
answered by the user. Re-diagnosing something already in `DECISIONS.md` is the most
expensive waste: if a problem smells familiar, look there first.
