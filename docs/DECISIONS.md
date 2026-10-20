# Decision log

Format: decision, why, discarded alternatives, consequence. Decisions
are reviewed only with new evidence, not by taste.

## D1 — Host in Rust + embedded Chromium (CEF), not Electron

`tauri-apps/cef-rs` was chosen (CEF bindings, maintained, x86_64/ARM64, Linux/macOS/
Windows) from day one, instead of an Electron client.

- **Why**: speed as a priority; a single language for host and core; fine
  control of Chromium's command line; and the Rust core serves the
  future native client as is.
- **Alternatives**: Electron (faster to start, but it drags Node along and more
  consumption); 100 % native client from scratch (months before being able to play).
- **Consequence / escape rule**: if CEF fails at gate F0, the fallback
  (Electron host) touches **only** `crates/host/`; `core/`, `page/` and the interception
  survive. That is the reason for isolating the host in a crate.

## D2 — Chromium sandbox disabled in F0

`Settings.no_sandbox = 1` and it is built without the `sandbox` feature.

- **Why**: the sandbox on Linux requires the setuid/zygote helper and complicates
  startup in dev; F0 seeks to validate viability, not hardening.
- **Accepted risk**: remote content runs without a process sandbox.
- **Re-evaluation**: mandatory in F9 (packaging), before distributing.

## D3 — Bridge over local WebSocket with token

See ARCHITECTURE.md §2. Random 16-byte token injected by
`__HAXFAST_BOOT`; it is validated in the first message.

- **Alternatives**: `cefQuery`/MessageRouter (native but coupled to CEF in both
  processes), Unix sockets (safer, but the browser does not see them).
- **Consequence**: any local process that guesses port and token can
  talk to the bridge. Ephemeral ports + token make it unviable in practice;
  if some day something sensitive is exposed, it migrates to a Unix socket.

## D4 — Preferences in Rust, versioned JSON file

`core/src/prefs.rs`: `Preferences { version, active_profile, profiles[] }`.

- **Why**: the preferences also feed the native track; validating and
  saving is pure logic, not DOM. Atomic save (`tmp` + `rename`) and
  validation with a list of problems, not only the first one.
- **Fail-open**: corrupt or invalid file ⇒ defaults + a warning in the log, never
  a crash nor file loss.
- **Profile**: includes what the game reads from `localStorage` (`player_name`,
  `avatar`, `extrapolation`, `player_auth_key` key, `geo_override`) because F4
  switches profile by writing that key before the game loads.

## D5 — Interception of `game-min.js`, download at startup, fail-open patches

- **Why**: the official engine keeps running as is; bytes are only prepended to it
  and point replacements are applied. No content-fingerprints or version
  matrices: one anchor per patch and, if it is not there, that patch is not applied.
- **Consequence**: if the site changes, the worst case is playing without improvements (and a
  warning in the log), never being unable to enter.
- **Discarded**: vendoring `game-min.js` (it goes stale and violates the intention of
  "official engine"); rewriting the bundle.

## D6 — Injection of the page bundle on every `on_load_end` of the main frame

With `execute_java_script`, in the main world.

- **Why**: the development cycle is "rebuild + F5"; there is never a bundle
  cached from a previous version.
- **Consequence**: the injection must be idempotent (the page layer protects its
  own state; from F3 on it is mounted only once per document).

## D7 — Logs to file with simple rotation

`~/.config/haxfast/logs/haxfast.log` (2 MiB, one backup) + `cef.log` for CEF.

- **Why**: being able to reconstruct what happened in a session without extra
  instrumentation. `no tracing` for now: two functions are enough.
- **Review**: if the volume grows, migrate to the `tracing` crate.

## D8 — Performance flags on the command line

In `on_before_command_line_processing`: `--disable-frame-rate-limit`,
`--disable-gpu-vsync` (applied only in the browser process; Chromium propagates to
the subprocesses).

- **Why**: these are the ones the HaxBall ecosystem uses to unlock the frame
  rate; our own governor (F2) verifies and compensates them.
- **Consequence**: the real effect is measured with the FPS HUD (and the startup
  sampling the page reports in F0).

## D9 — No Chrome extensions

CEF does not load Chrome extensions in a normal way (only with the experimental
"Chrome runtime"). No vendoring the All-in-One extension: the useful features
are ported as our own modules — auto-join (F7), kick/ban (F8) — and the chat
ones only if they are requested.

## D10 — Linux only first

AppImage/deb in F9. Windows/macOS remain documented (cef-rs supports them) but
nothing is tested and no platform-specific code is maintained until it is requested.

## D11 — GPL-3.0 license, no CI

The license is GPL-3.0-only for compatibility with the ecosystem (reference code and
extension are GPL). CI pipelines are out of scope for now; the repo is published on
GitHub per D16.

## D12 — The wasm adapter is born in F5

When there is core logic the page needs (e.g. shortcuts with state),
`crates/wasm` is added. Before that it would be an empty crate: it is not created.

## D13 — CSS-only skins and replaceable sounds (F3/F6)

- Skins: folder with `skin.json` + CSS (`tokens.css`, `shell.css`, `game.css`) +
  `assets/`. No executable JS in skins.
- Sounds: byte replacement by interception (same pipeline as `game-min.js`),
  validation by magic bytes (`RIFF/WAVE`, `OggS`, `ID3`/MPEG, `fLaC`), ~5 MB per
  file, in `userData/sounds/`. Changes to the file are applied on reload
  (a live reload kicks you out of the room).

## D14 — Navigating to the official site (`/play`), not to the resolved `game.html`

The host loads the requested URL (`https://www.haxball.com/play`); resolving
`game.html` only serves to locate `game-min.js`.

- **Why**: `game.html` loaded as the main document stays at "Loading":
  it waits for its parent page, which embeds it in an iframe. The reference client
  (Electron) also loads `/play`.
- **Consequence**: the game lives in the `gameframe` iframe (same origin); the bundle
  is still intercepted by URL and the bridge is injected in the top frame, with
  access to the game through `contentDocument`. Reverts the previous decision of
  "navigating straight to the game"; measured on 18-10-2026.

## D15 — The interception always returns `ReturnValue::CONTINUE`

`on_before_resource_load` returns an explicit `CONTINUE` (in `crates/host/app.rs`).

- **Why**: in cef-rs, `ReturnValue::default()` is `RV_CANCEL`, and so is the
  trait's own default. If the handler returns the default, CEF **cancels** the request
  and never calls `GetResourceHandler`: the patch is not served and on top of that
  page resources are lost (the game is left without `game-min.js`).
- **Consequence**: any `ResourceRequestHandler` of our own must make the return value
  explicit; the comment in `app.rs` stays so as not to reintroduce the bug.

## D16 — The repo is published on GitHub (public, GPL-3.0)

`https://github.com/glospi/Haxfast` is the canonical remote, public, with the GPL-3.0
license already in the repo. It reverses the "local repo" part of D11.

- **Why**: the project goes public; the README, docs and code are the entry point for
  anyone reading it, and a remote also protects the history.
- **Alternatives**: staying local (D11) or a private repo — discarded once publication
  was decided.
- **Consequence**: everything pushed is public. The REA material (`docs/reference-me/`,
  analysis of the official game and of another client) stays local and git-ignored:
  it is not part of the published repo. No CI, no releases yet.

## D17 — The project language is English

Code, comments, documentation and commit messages are in English.

- **Why**: it is a public repo and the HaxBall/CEF ecosystem material is in English;
  mixing languages in a repo that strangers read costs more than translating once.
- **Alternatives**: Spanish (the previous rule) or bilingual docs — discarded.
- **Consequence**: the log strings and the verification recipe in `docs/GATE-F0.md` and
  `AGENTS.md` are in English too; they must stay in sync with the code.

## D18 — Brace style: Allman by hand in the page layer, default in Rust

The page layer (TS/JS) is fully Allman (block braces on their own line), kept by hand —
there is no formatter there and nothing fights the style. In Rust, Allman was tried and
discarded.

- **Why**: the user's readability goal was met where it is safe; in Rust it cannot be
  enforced reliably.
- **Alternatives**: rustfmt `brace_style = "AlwaysNextLine"` — nightly-only (the
  system's stable rustfmt ignores it and warns), reaches item braces only (`fn`,
  `struct`, `impl`, …) with no option for `if/else/match/loop` braces, and any future
  stable `cargo fmt` would silently revert the items. Installing rustup+nightly just
  for a partial style was rejected too. dprint for TS was skipped: three small files,
  no new dependency for that.
- **Consequence**: TS/JS keeps Allman by hand; Rust stays on the default rustfmt style
  with no `rustfmt.toml` and no nightly dependency. Hand-moving Rust braces into
  Allman is not allowed — the next `cargo fmt` would undo it.

## D19 — Disk cache stores the ORIGINAL bundle, keyed by the deploy URL

`$HAXFAST_HOME/engine/` holds one `<id>.js` (original bytes) + `<id>.json`
(`{url, hash}`) pair per deploy; the id is a hash of the bundle URL and the hash is
a content digest computed with std `DefaultHasher` (no new dependency). A hit
requires both the URL and the recomputed content hash to match.

- **Why**: the bundle URL rotates per deploy and the game must re-download it every
  start; the deploy resolution stays live (the `/play` page decides), only the 170 KB
  payload is reused. The patched output is NOT cached: anchors are re-evaluated every
  startup, so a new HaxFast automatically re-patches the same cached original.
- **Alternatives**: caching the patched bundle (rejected: it would re-serve stale
  patches when HaxFast updates); trusting the file without a content hash (rejected:
  a truncated file would corrupt the game).
- **Consequence**: corrupt or foreign entries are misses (logged WARN) and the
  network takes over; deleting the folder is always safe. Deploy rotation leaves
  orphan pairs behind until the folder is cleaned manually (acceptable at 170 KB).

## D20 — Engine toggles live in preferences and are read by the runtime at call time

The four patch toggles are a new optional `engine` section in `preferences.json`
(all default ON, old files survive by serde default) and each patched call site
consults `__HAXFAST_ENGINE__.on(name)` **live**, so `set(name, false)` from the
console changes the behavior of the running game without a reload.

- **Why**: the user must be able to A/B a patch instantly (it is a performance tweak,
  and a bad one must be diagnosable in one line) while the startup default stays a
  plain file the UI of F2 will edit.
- **Alternatives**: patch-time only (`set` needed a reload) or a runtime object with
  no persistence — both worse for diagnosis; a server-side default list is not for
  this project (D5).
- **Consequence**: a `set()` change is not persisted (next start re-reads
  `preferences.json`); persistence of user changes is F2 HUD work. The runtime is
  still injected with all toggles off — the state stays inspectable.

## D21 — Governor: `gsync` by default; `locked`/`unlimited` unlock Chromium

`video.governor` is `gsync` (default), `locked` or `unlimited`; the mode and the
GPU backend are startup choices. `locked`/`unlimited` add
`--disable-frame-rate-limit` and `--disable-gpu-vsync`; `gsync` leaves Chromium's
defaults. `locked` also asks the engine runtime for a draw cap (`video.fps_limit`,
10–1000, default 144); `unlimited` has no cap. The HUD shows a restart note when
the mode or backend changes.

- **Why**: `gsync` is the balanced default and the path the official client uses;
  unlocking without a cap (the old ecosystem recipe) spends GPU and heat for no
  benefit, while a cap without the flags cannot go above the compositor's ~60.
  The two knobs together are the smallest set that gives all three behaviors.
- **Alternatives**: flags only (no control), cap only (not a real unlock), default
  `unlimited` (waste on laptops) or default `locked` (arbitrary target before the
  user chooses one).
- **Consequence**: switching mode/backend needs a restart; the HUD and the log
  say so. The strings are parsed leniently (unknown → default) so a hand-edited
  file never invalidates the profile; the host logs the effective flags at
  startup. The cap itself is live (D22).

## D22 — The frame cap skips draws only, never simulation, network or input

The F2 `frame-governor` patch routes the live-view draw guard through
`__HAXFAST_ENGINE__.skipDraw(now, nativeBlocked)`. It returns `true` only when the
draw must be skipped: the game's native FPS Limit (`nativeBlocked`, passed
through untouched) or the governor's interval minus a 10% early tolerance. Every
other line of the loop (simulation, network, input, scheduling) still runs at the
monitor cadence. The toggle off, a missing anchor or an exception returns
`nativeBlocked`, exactly vanilla.

- **Why**: slowing simulation would change the match and the netcode; the goal is
  frame pacing and GPU cost. The game's own 30 FPS Limit already works this way,
  so reusing the same guard is both minimal and predictable.
- **Alternatives**: throttling the whole rAF loop (slows simulation), patching the
  renderer entry (more anchors, more risk), trusting `--disable-gpu-vsync` alone
  (no cap), a host-side timer (cannot reach the draw safely).
- **Consequence**: the game's own FPS counter and our metric both count draws; a
  30 cap does not slow the match clock. The replay/menu loop has a different
  anchor and stays untouched, so the HUD falls back there. The 10% tolerance
  makes the cap a **ceiling**: without it, 30 fps on a 60 Hz loop alternated
  20/30 because of timestamp jitter; with it, the average can exceed the target
  slightly (measured 32.5 for a 30 target, 140 for 144).

## D23 — HUD lives in the top frame, is visible at startup and persists its settings

`page/src/hud.ts` renders a bottom-right overlay in the **top** frame (the bridge
and the future shell live there, D14); it reads the game only through
`contentWindow.__HAXFAST_ENGINE__`. `video.show_hud` defaults to `true`; `F8`
toggles it provisionally (F5 turns binds into configuration). The panel edits the
governor and limit, the GPU backend, HUD visibility and the engine toggles; the
governor and backend need a restart, the limit and toggles apply live. Every
change is saved with `prefs.set` into `video`/`engine`.

- **Why**: the requirement was a HUD visible at startup with an immediate hide
  key, without adding UI inside the game iframe (it fights the game's own hooks
  and would be lost on its reloads). Persisting avoids re-setting the same
  options every launch.
- **Alternatives**: HUD inside the game iframe (fragile), a native second window
  (focus/cost), hidden by default (contradicts the requirement), `localStorage`
  (the host file is the source of truth).
- **Consequence**: F8 is explicitly provisional until F5; the key listener is
  installed in the top frame and the game window and ignores keystrokes while
  typing. The HUD is fail-open: no game frame, no HUD, the game keeps playing.
  The full WebGL renderer string is shown so a silent backend fallback is visible.

## D24 — Metrics are the engine's real draws; 256-interval window; rAF fallback marked `~`

The engine runtime records the interval between **actual draws** (inside
`skipDraw`), keeps the last 256 intervals and exposes
`frameStats() → {fps, p95, samples, draws, limit}`. p95 is nearest rank; intervals
≥ 250 ms are discarded (tab resumes, breakpoints); the window resets when the page
is hidden and when the cap changes; if no draw happened for 500 ms the stats
report 0 samples. Without the runtime, with the patch off, or with no recent
draws, the page samples `requestAnimationFrame` inside the game frame and the HUD
marks the number `~` (approximate). The bridge method is `metrics.frame`, logged
every 5 s.

- **Why**: rAF measures loop cadence, not draws: under a draw cap or the game's
  native 30 it would report 60 and lie. The engine counters are the only honest
  source and they already know the cap and the native guard. 256 intervals keep
  the HUD responsive (≈4 s at 60, ≈8.5 s at 30); the first 2048-window version was
  rejected after a native-30 change took over a minute to appear.
- **Alternatives**: rAF only (lies), Chromium tracing/`--enable-stats` (heavy and
  outside the page), reading the renderer's timestamps (not exposed), a fixed
  time window with per-sample timestamps (more state for little gain at these
  rates).
- **Consequence**: the HUD's fps is the draw rate, not the simulation tick rate;
  `~` is explicitly approximate and not comparable to `engine` rows. A stale
  counter is never shown as a live 60. The cap's 10% tolerance means the average
  can be slightly above the target; p95 is draw interval, not input latency.

## D25 — Ads are canceled at the request handler; shell chrome stays until F3

The official `/play` shell injects cpmstar in `div.rightbar`; it pulls ~30 ad and
cookie-sync hosts (doubleclick, googlesyndication, rubicon, smartadserver, …).
`host/ads.rs` suffix-matches those hosts and returns a dedicated
`ResourceRequestHandler` whose `on_before_resource_load` returns
`ReturnValue::CANCEL`. The page layer hides the empty `div.rightbar`. The default
menu is reduced to Copy/Paste; the page layer tracks the right-clicked editable
element because the native menu steals DOM focus and the edit commands are no-ops
without it.

- **Why**: this is the only measurable Chromium reduction left without a custom
  CEF build (the build needed 16 GB RAM and 120–150 GB of disk in this machine's
  setup). Ads are outside the game, cost network/CPU/RSS, and the official
  sidebar is not part of HaxBall. Host-suffix matching survives deploy URL
  changes and path obfuscation.
- **Alternatives**: custom CEF build or source strip (not viable here), DNS/hosts
  file (global side effects), CSS-only hiding (ads still load), strict network
  allowlist (breaks reCAPTCHA and future game endpoints).
- **Consequence**: the list needs maintenance when ad networks rotate; keep it
  evidence-based (logs + REA). Never add `haxball.com`, `p2p.haxball.com`,
  `google.com`/`gstatic.com` (reCAPTCHA), `fonts.googleapis.com` or loopback. Unlike
  D15, this handler intentionally returns `RV_CANCEL`. The top header/navbar and
  external-link policy remain F3. Measured effect in
  [PERFORMANCE.md](PERFORMANCE.md#shell-ads-f2-hardening).
