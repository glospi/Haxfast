# Gate F0 — Does CEF hold up as a host?

Objective: verify end to end that embedded Chromium can load the official
game, serve it an intervened `game-min.js`, talk to the page and enter a real
room. Everything else in the project depends on this.

## How to run

```sh
npm install && npm run build:page
cargo run --manifest-path native/Cargo.toml -p haxfast-host
# or directly, after building:
./native/target/debug/haxfast-host
```

The binary carries RUNPATH `$ORIGIN`, so `libcef.so` (copied by cef-dll-sys
next to the executable) finds itself: there is no need to touch `LD_LIBRARY_PATH`.

Logs: `~/.config/haxfast/logs/haxfast.log` and `cef.log`. With
`HAXFAST_REMOTE_DEBUG=9222` the page can be inspected at
`http://127.0.0.1:9222` (useful for the "verification" column).

## Checklist

| # | What is verified | How it looks | Status |
| - | --- | --- | --- |
| 1 | The host starts and creates the window | Log: `cef: window created`; the window appears | ✔ 18-10-2026 20:39 (a single 1280×720 window, no Chrome UI) |
| 2 | The official page loads with the Chrome UA | Log: `page: loaded https://www.haxball.com/play`; the site is visible | ✔ 18-10-2026 20:39 (HTTP 200) |
| 3 | `game-min.js` is served by HaxFast | Log: `engine: game-min.js ready (… patches: runtime, f0-marker)` and `engine: intercepted …game-min.js` | ✔ 18-10-2026 20:39; `fetch("game-min.js")` from the page returns 173 243 B with `__HAXFAST_ENGINE__` and `/*haxfast:f0*/` |
| 4 | The game starts (main menu) | The HaxBall menu is visible | ✔ engine in the game iframe + "Choose nickname" dialog (screenshot `/tmp/opencode/juego-f0.png`); visual confirmation from the user when entering the menu is missing |
| 5 | The page bundle is injected and the bridge responds | Log: `page: bundle injected (dist/page.js)`, `[page] page bootstrap`, `[page] prefs received` | ✔ 18-10-2026 20:39 + `prefs.set`/`prefs.get` roundtrip over CDP |
| 6 | The frame-rate flags go through CEF | Log: `[page] fps sample {…}` with fps > 60 (ideally 200+) | ✔ the flags arrive (`cef: startup flags added (frame-rate)`); 2296 fps on 18-10-2026 20:27 with this build. With the monitor active, the desktop compositor limits to ~58-60 (see notes) |
| 7 | Enter a public room and play | Join via UI; WebRTC/STUN log with no fatal errors; the match is visible | pending (manual) |
| 8 | reCAPTCHA does not block | Login/entry with no blocked captcha | pending (manual) |
| 9 | Audio works | Hits/goals are heard | pending (manual) |
| 10 | Fullscreen | The game button enlarges the window to fullscreen | pending (manual) |

Notes:

- Navigating to `https://www.haxball.com/play`, not to the bare `game.html`: loaded as the
  main document, the game stays at "Loading" (it waits for its parent page, which
  embeds it in an iframe). The bundle is resolved and intercepted all the same (D14).
- Item 6: the ~58-60 fps cap comes from the environment, not from the game nor from the
  flags: with the monitor active, `about:blank` in this same instance also measures
  58 fps (vsynced presentation of the desktop compositor). On 18-10-2026 20:27, with the
  same build, the sampling gave 2296 fps. The real unlock inside the game is
  resumed in F2 (governor).
- Items 7–10: manual with a real room. Write down the result and the log time here.
- `window.__HAXFAST_BOOT` and `__HF_BRIDGE__` remain visible in devtools to
  verify the bridge by hand (`__HF_BRIDGE__.call("prefs.get")`). The bridge lives in
  the top frame; the game is in the `gameframe` iframe (same origin).
- The advertising notices of `/play` are visible inside the window; hiding them by CSS
  is skins work (F3), not gate work.

## Failure criterion

It is considered a **hard failure** if Chromium does not load the page, if there is no WebRTC (a
room cannot be entered) or if the bridge turns out to be impossible to implement. In that
case: stop, write the failure in this file and ask the user before
applying the fallback (Electron host, only `crates/host/`).

## Sandbox status

In F0 it runs with `no_sandbox` (see DECISIONS.md D2). It is not part of the gate:
it is declared debt that is paid in F9.
