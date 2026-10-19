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
