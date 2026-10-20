// HaxFast — page layer (injected into the top frame).
// F2: frame metrics, the FPS governor and the HUD over the official page.
// Fail-open: any error here leaves the official client exactly as it was.

import { bridge } from "./bridge";
import { engineOf, findGameFrame, type GameFrame } from "./game";
import { applyGovernor } from "./governor";
import { Hud, installHotkey } from "./hud";
import { RafSampler, selftest, snapshotOf } from "./metrics";
import { asGovernor, clampFps, type EngineRuntime, type Preferences, type VideoPrefs } from "./types";

const log = (msg: string, data?: unknown): void => bridge.notify("log", { msg, data });
const TICK_MS = 250;
const METRICS_LOG_MS = 5000;

declare global
{
  interface Window
  {
    __HF_SELFTEST__?: () => string[];
    __HF_CTX_TARGET__?: EventTarget | null;
  }
}

// F2: the native context menu steals DOM focus, and Copy/Paste are no-ops
// without it. Remember the right-clicked element so the host can refocus it
// before running the menu command.
function trackContextTarget(win: Window): void
{
  win.document.addEventListener("contextmenu", () =>
  {
    const active = win.document.activeElement;
    win.__HF_CTX_TARGET__ = active && active !== win.document.body ? active : null;
  }, true);
}

async function main(): Promise<void>
{
  if (window !== window.top) return; // top frame only (D14)

  hideAds();
  trackContextTarget(window);

  const connected = await bridge.connect();
  log("page bootstrap", { href: location.href, connected });

  const prefs = normalize((await bridge.call("prefs.get")) as Preferences | undefined);
  log("prefs received", { profiles: prefs.profiles.length, governor: prefs.video.governor });

  const sampler = new RafSampler();
  let game: GameFrame | undefined;
  let runtime: EngineRuntime | undefined;
  let appliedTo: EngineRuntime | undefined;
  let lastLogAt = 0;

  const save = (): void =>
  {
    void bridge.call("prefs.set", prefs).catch((error: unknown) => log("prefs save failed", String(error)));
  };

  const hud = new Hud(prefs, {
    engine: () => runtime,
    onVideo: (video) =>
    {
      applyGovernor(runtime, asGovernor(video.governor), video.fps_limit, log);
      save();
    },
    onEngineToggle: (name, value) =>
    {
      runtime?.set(name, value);
      save();
    },
  });
  hud.setVisible(prefs.video.show_hud);
  installHotkey(hud, document);

  window.__HF_SELFTEST__ = selftest;
  if (location.hash.includes("selftest"))
  {
    const failures = selftest();
    log("selftest", { failures: failures.length, detail: failures });
  }

  window.setInterval(() =>
  {
    try
    {
      const frame = findGameFrame();
      if (!frame)
      {
        if (game)
        {
          game = undefined;
          runtime = undefined;
          appliedTo = undefined;
          sampler.detach();
        }
      }
      else
      {
        // The WindowProxy survives iframe navigations; the document does not.
        if (frame.window !== game?.window || frame.document !== game?.document)
        {
          sampler.detach();
          installHotkey(hud, frame.document);
          trackContextTarget(frame.window);
          sampler.attach(frame.window);
        }
        game = frame;
        runtime = engineOf(frame);
      }

      if (runtime && runtime !== appliedTo)
      {
        appliedTo = runtime;
        applyGovernor(runtime, asGovernor(prefs.video.governor), prefs.video.fps_limit, log);
      }

      const snapshot = snapshotOf(runtime, sampler);
      const mode = asGovernor(prefs.video.governor);
      const limit = mode === "locked" ? clampFps(prefs.video.fps_limit) : 0;
      const cap = runtime?.frameLimit ?? 0;
      const native30 = nativeFpsLimit(game);

      hud.render({ ...snapshot, mode, limit, cap, native30 });

      const now = performance.now();
      if (now - lastLogAt >= METRICS_LOG_MS)
      {
        lastLogAt = now;
        bridge.notify("metrics.frame", {
          fps: round1(snapshot.fps),
          p95: round1(snapshot.p95),
          samples: snapshot.samples,
          source: snapshot.source,
          mode,
          limit,
          cap,
          gpu: hud.renderer,
          patches: runtime?.patches ?? [],
          native30,
        });
      }
    }
    catch (error)
    {
      console.warn("[haxfast] hud tick failed", error);
    }
  }, TICK_MS);
}

function normalize(raw: Preferences | undefined): Preferences
{
  const defaults: Preferences = {
    version: 1,
    active_profile: "default",
    profiles: [],
    engine: {
      canvas_resize_cache: true,
      stadium_layer_cache: true,
      zero_input_tolerance: true,
      hidden_frame_skip: true,
      frame_governor: true,
    },
    video: { governor: "gsync", fps_limit: 144, show_hud: true, gpu_backend: "auto" },
  };
  if (!raw) return defaults;
  const video: VideoPrefs = { ...defaults.video, ...(raw.video ?? {}) };
  video.fps_limit = clampFps(video.fps_limit);
  return {
    ...defaults,
    ...raw,
    engine: { ...defaults.engine, ...(raw.engine ?? {}) },
    video,
  };
}

// F2: `div.rightbar` is the official shell's ad sidebar; the host cancels the
// requests and this hides the empty strip (fail-open: layout still works if the
// class disappears).
function hideAds(): void
{
  const style = document.createElement("style");
  style.textContent = "div.rightbar{display:none!important}";
  (document.head ?? document.documentElement).appendChild(style);
}

function nativeFpsLimit(game: GameFrame | undefined): boolean
{
  try
  {
    return game?.window.localStorage.getItem("fps_limit") === "1";
  }
  catch
  {
    return false;
  }
}

function round1(value: number): number
{
  return Math.round(value * 10) / 10;
}

void main().catch((error: unknown) =>
{
  console.error("[haxfast] page init failed", error);
  log("page init failed", String(error));
});
