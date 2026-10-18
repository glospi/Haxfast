# Roadmap

Every phase has an exit criterion. No phase starts before the
previous one closes (except track R, which runs alongside them from F2).

## F0 — CEF viability gate *(closed 18-10-2026 — all items green)*

Exit criterion: [GATE-F0.md](GATE-F0.md) green. If it fails hard, the user
is consulted and the fallback is applied (Electron host), which touches only `crates/host/`.

## F1 — Engine performance patches *(current phase)*

Four fail-open patches over `game-min.js`, each with an individual toggle and
visible state:

1. `canvas-resize-cache` — avoids recalculating the canvas layout when the size
   did not change.
2. `stadium-layer-cache` — caches the static layers of the stadium.
3. `zero-input-tolerance` — input tolerance 0 (immediate response).
4. `hidden-frame-skip` — skips render when the window is not visible.

Plus: on-disk cache of the original bundle (hash + `userData/engine/`),
`__HAXFAST_ENGINE__` runtime with patch state, and [docs/ENGINE.md](ENGINE.md).

## F2 — Speed and metrics

FPS governor (gsync/locked/unlimited style modes), minimal HUD (FPS, frame
p95, patch state, hotkey to hide), real frame time sampling, and
GPU backend selector (includes `vulkan` via ANGLE, see [VULKAN.md](VULKAN.md)).
The HUD lives in JS: it measures against `requestAnimationFrame`, which is the only honest
source.

## F3 — Shell UI and skins

Own UI (ES/EN, dark and minimal by default) mounted over the page; CSS-only
skins system (folder or zip, import/duplicate/export, manual hot reload);
[SKINS.md](SKINS.md). External links open in the system browser.

## F4 — Profiles and authentication

Full profiles over `core::prefs` (change identity, avatar, extrapolation,
`playerauth` key), with the CEF session persisted in `userData`.

## F5 — Shortcuts

Configurable keyboard shortcuts: key → command or message (superset of what
other clients offer). The `wasm` crate is born if the page needs core logic.

## F6 — Sounds

Replacement of the game's 7 sounds (`chat`, `highlight`, `kick`, `goal`, `join`,
`leave`, `crowd`) with validation by magic bytes and per-category volume live;
[SOUNDS.md](SOUNDS.md).

## F7 — Rooms and favorites

Own list of favorite rooms with live presence, per profile, and auto-join at
startup.

## F8 — Game view skin and administration

Skin over the game view (room HUD inside the iframe) and administration
modules: kick/ban. Highlighting of favorites inside the official list if it
still makes sense after F7.

## F9 — Packaging

AppImage (and `.deb`) `haxfast-*.AppImage`, docs update, re-evaluation of the
sandbox (D2) and of the CEF bundle.

## Track R — Native client (running alongside from F2)

Long-term path, with no end date; see [NATIVE-TRACK.md](NATIVE-TRACK.md):

1. REA capture of the room handshake (signaling `wss://p2p.haxball.com`).
2. Signaling CLI: create/join rooms without a browser.
3. Headless join with `webrtc-rs` + own simulation in `core`.
4. Gate E3: client without Chromium that enters rooms and shows the match.
