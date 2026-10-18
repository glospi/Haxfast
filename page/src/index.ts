// HaxFast — page layer (injected into the game page).
// F0: minimal bootstrap: connect the bridge, read prefs and sample raw FPS for the gate.
import { bridge } from "./bridge";

const log = (msg: string, data?: unknown) => bridge.notify("log", { msg, data });

async function main(): Promise<void>
{
  if (window !== window.top) return; // F0: top frame only; the game iframe arrives in F1/F2

  await bridge.connect();
  log("page bootstrap", { href: location.href });

  const prefs = (await bridge.call("prefs.get")) as { profiles?: unknown[] } | undefined;
  log("prefs received", { profiles: prefs?.profiles?.length ?? 0 });

  sampleFps(5000);
}

// Raw FPS sampling: with the unlocked frame-rate flags it should exceed 60;
// it is the simplest signal to know whether the switches made it through CEF.
function sampleFps(ms: number): void
{
  let frames = 0;
  const start = performance.now();
  const tick = (): void =>
  {
    frames++;
    const elapsed = performance.now() - start;
    if (elapsed < ms)
    {
      requestAnimationFrame(tick);
    }
    else
    {
      const fps = Math.round(((frames * 1000) / elapsed) * 10) / 10;
      log("fps sample", { fps, seconds: Math.round(elapsed / 100) / 10 });
    }
  };
  requestAnimationFrame(tick);
}

void main();
