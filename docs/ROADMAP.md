# Roadmap

Every phase has an exit criterion. No phase starts before the
previous one closes (except track R, which runs alongside them from F2).

## F0 — CEF viability gate *(closed 18-10-2026 — all items green)*

Exit criterion: [GATE-F0.md](GATE-F0.md) green. If it fails hard, the user
is consulted and the fallback is applied (Electron host), which touches only `crates/host/`.

## F1 — Engine performance patches *(closed 19-10-2026 — shipped)*

Exit criterion met: four fail-open patches over `game-min.js` with individual
toggles and visible state, cache of the original bundle, and
[ENGINE.md](ENGINE.md). Details live there now (anchors, bake mechanics,
honest limits). F0's `/*haxfast:f0*/` marker patch and its anchor were replaced
by the real pipeline.

## F2 — Speed and metrics *(closed 19-10-2026 — shipped)*

Exit criterion met: three persisted governor modes (`gsync` default, `locked`
10–1000 FPS, `unlimited`), a draw-only cap that leaves simulation/network/input
alone, a HUD visible at startup with `F8` and persisted settings, real draw
metrics with a clearly marked rAF fallback, and an ANGLE backend selector that
shows the renderer actually active. [PERFORMANCE.md](PERFORMANCE.md) has the
design and the measured results; D21–D24 record the whys.

Honest limits: the cap is a ceiling (10% early tolerance), Vulkan was slower than
GL on the test machine, the menu/replay loop is not patched (fallback `~`), and
`F8` is provisional until F5 makes binds configurable.

F2 hardening (20-10-2026): the official shell's ad sidebar is gone (host blocklist
plus hidden `div.rightbar`), the context menu is Copy/Paste only, and the measured
effect is in [PERFORMANCE.md](PERFORMANCE.md#shell-ads-f2-hardening) (D25). The
official header/navbar and the external-link policy remain F3.

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
