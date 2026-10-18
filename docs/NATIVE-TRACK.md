# Native track — toward a HaxFast without Chromium

Long-term objective: a client that speaks the HaxBall protocol
directly and renders on its own, with the current `core` as a base. No date:
it is a track that advances when the path clears up, alongside the F phases.

## Why it exists

- Input latency and frame time under total control (today we depend on Chromium).
- Being able to run headless (bots, tools, room tests) without a browser.
- Not depending on the lifecycle of the official site.

## Central risk

HaxBall is P2P over WebRTC: entering a room requires the
"signaling" handshake against `wss://p2p.haxball.com` with the proprietary protocol. That
can be observed (REA) but it is not documented and can change.

## Stages

### R1 — Protocol capture

With the REA tools: capture an entry to a real room and document the
handshake, signaling messages, identity format (`player_auth_key`) and
creation of `RTCPeerConnection`. Output: report in `docs/reference/` (local,
git-ignored) + reusable captures.

### R2 — Signaling CLI

A crate of its own (outside `core`) that creates/lists/joins rooms by signaling using
`webrtc-rs`, without render. Output: a script that prints the state of a real room.

### R3 — Headless join

`core` receives the messages from the room host (format already documented by REA in
`docs/reference/`, local), applies simulation/extrapolation and keeps the match state
without drawing anything. Output: a full match running in the console.

### R4 — Gate E3: own minimal client

- Enters real rooms, shows the match and allows playing.
- `render/` crate with `wgpu` (see VULKAN.md), fed by the core state.
- Everything else (UI, skins, shortcuts) is reused from `page/` only as
  product reference: the native UI is another job.

## What does not change

The `core`. It is its reason to exist: if the native track needs to change the core to
advance (for example, adding a data type the simulation needs), it
is changed there and the CEF client **also** takes advantage of it.

## Status

Not started. R1 begins after F1 (when the engine patches are already
closed and gate F0 validated).
