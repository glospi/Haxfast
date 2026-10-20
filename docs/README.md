# HaxFast Documentation

Read in this order depending on what you are looking for:

| Document | What for |
| --- | --- |
| [ARCHITECTURE.md](ARCHITECTURE.md) | How the client is put together: layers, boundaries, data flow. |
| [ENGINE.md](ENGINE.md) | The `game-min.js` patches, runtime and toggles (F1–F2). |
| [PERFORMANCE.md](PERFORMANCE.md) | Frame governor, HUD, metrics and measured results (F2). |
| [DECISIONS.md](DECISIONS.md) | Why each thing is the way it is (alternatives and consequences). |
| [ROADMAP.md](ROADMAP.md) | Phases F0–F9, native track (R) and exit criteria. |
| [GATE-F0.md](GATE-F0.md) | F0 verification checklist (historical; green). |
| [VULKAN.md](VULKAN.md) | What can and cannot be expected from Vulkan; F2 measurement. |
| [NATIVE-TRACK.md](NATIVE-TRACK.md) | Path to the 100 % native client (no Chromium). |

Documents that are born in later phases: `SKINS.md` (F3), `SOUNDS.md` (F6).

`reference-me/` is **not versioned**: the REA reports (official game and another
client) and the loose notes stay on the machine that wrote them, git-ignored. They
are reference material: they describe the state of the world at the time of
writing, not the HaxFast architecture.
