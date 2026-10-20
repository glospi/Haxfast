# HaxFast

High-performance HaxBall client. Host in Rust with embedded Chromium (CEF) and
a page layer (TypeScript → injected JS) that lives on top of the official site.

## Status

**F0 closed on 18-10-2026** — [docs/GATE-F0.md](docs/GATE-F0.md) is green: CEF
loads the official game, HaxFast serves the patched `game-min.js`, the page bridge
works and real rooms can be played (WebRTC).

**F1 closed on 19-10-2026** — [docs/ENGINE.md](docs/ENGINE.md): four fail-open
performance patches over `game-min.js` (input tolerance 0, cached canvas resize,
cached stadium layers, skip drawing while hidden), each toggleable live from
`__HAXFAST_ENGINE__`, plus a disk cache of the original bundle.

**F2 closed on 19-10-2026** — [docs/PERFORMANCE.md](docs/PERFORMANCE.md): frame-rate
governor (`gsync` default, `locked` with a live draw cap, `unlimited`), a HUD with
real draw metrics and p95 (`F8` toggles it), persistent video/engine settings and an
ANGLE backend selector with the actual renderer shown. The cap skips draws only;
simulation, network and input keep their cadence. Current phase: **F3 (shell UI and
skins)** — see [docs/ROADMAP.md](docs/ROADMAP.md).

## Structure

```
native/            Rust workspace
  crates/core/     Pure logic (no CEF, no DOM) — seed of the native client
  crates/host/     Binary: CEF, window, interception, bridge, logs
page/              Page layer (TS): compiles to dist/page.js and is injected
build/             Build scripts (esbuild; dev only, no runtime)
docs/              Project documentation (start with docs/README.md)
resources/         Client resources (skins, etc., from F3)
```

## Build and run (Linux)

Requires stable Rust, Node ≥ 20 (only to build the page), `cmake`, `ninja`,
`clang` and the GTK3/NSS libs. The first host build downloads the CEF
binaries (~200 MB) and compiles the C++ wrapper: it can take several minutes.

```sh
npm install
npm run build:page          # dist/page.js
cargo run --manifest-path native/Cargo.toml -p haxfast-host
```

Useful variables:

| Variable | Effect |
| --- | --- |
| `HAXFAST_HOME` | Data folder (by default `~/.config/haxfast`) |
| `HAXFAST_URL` | Page to load (by default `https://www.haxball.com/play`) |
| `HAXFAST_REMOTE_DEBUG=9222` | Chromium remote debugging port |
| `HAXFAST_PAGE_BUNDLE` | Path to the page bundle (by default `dist/page.js`) |

## Documentation

- [docs/README.md](docs/README.md) — index.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — how the client is put together.
- [docs/ENGINE.md](docs/ENGINE.md) — game-min.js patches, runtime and toggles.
- [docs/PERFORMANCE.md](docs/PERFORMANCE.md) — governor, HUD, metrics, measured results.
- [docs/DECISIONS.md](docs/DECISIONS.md) — decision log and whys.
- [docs/ROADMAP.md](docs/ROADMAP.md) — phases F0–F9 and native track.
- [docs/For agents/AGENTS.md](docs/For%20agents/AGENTS.md) — operating guide for agents working on the repo.

## License

GPL-3.0-only (see [LICENSE](LICENSE)). It was chosen for compatibility with the
HaxBall client/extension ecosystem, whose reference code is GPL.
