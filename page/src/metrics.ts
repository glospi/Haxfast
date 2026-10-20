// Frame metrics. The engine's draw counters are the honest source (they count
// the frames the game actually renders, after the native FPS Limit and the F2
// governor). When the engine patch is missing, this falls back to a rAF
// sampler in the game frame; the HUD marks that fallback as approximate.

import { frameLimit } from "./types";
import type { EngineRuntime } from "./types";

const RING = 256;
const MAX_FRAME_MS = 250; // longer gaps are tab resumes or breakpoints, not frames

export interface MetricsSnapshot
{
  fps: number;
  p95: number;
  samples: number;
  source: "engine" | "raf";
}

export function percentile(ring: Float32Array, filled: number, quantile: number): number
{
  if (filled <= 0) return 0;
  const sorted = Array.from(ring.subarray(0, filled)).sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(quantile * sorted.length) - 1));
  return sorted[index];
}

export function meanFps(ring: Float32Array, filled: number): number
{
  if (filled <= 0) return 0;
  let sum = 0;
  for (let i = 0; i < filled; i++) sum += ring[i];
  const mean = sum / filled;
  return mean > 0 ? 1000 / mean : 0;
}

export function snapshotOf(runtime: EngineRuntime | undefined, sampler: RafSampler): MetricsSnapshot
{
  if (
    runtime?.patches?.includes("frame-governor")
    && runtime.on("frame-governor")
    && typeof runtime.frameStats === "function"
  )
  {
    const stats = runtime.frameStats();
    // No recent draws (replay loop, hidden view): the engine counters would
    // freeze, so fall back to the loop cadence until draws resume.
    if (stats.samples > 0) return { fps: stats.fps, p95: stats.p95, samples: stats.samples, source: "engine" };
  }
  return { ...sampler.stats(), source: "raf" };
}

/** rAF cadence of the game frame: only used when the engine counters are absent. */
export class RafSampler
{
  private win: Window | undefined;
  private running = false;
  private last = 0;
  private index = 0;
  private filled = 0;
  private intervals = new Float32Array(RING);

  attach(win: Window): void
  {
    if (this.win === win && this.running) return;
    this.detach();
    this.win = win;
    this.running = true;
    this.reset();
    try
    {
      win.document.addEventListener("visibilitychange", this.onVisibility);
    }
    catch
    {
      // The frame is already gone: the tick will notice and stop (fail-open).
    }
    this.tick();
  }

  detach(): void
  {
    if (this.win && this.running)
    {
      try
      {
        this.win.document.removeEventListener("visibilitychange", this.onVisibility);
      }
      catch
      {
        // The document is gone; there is nothing to remove.
      }
    }
    this.running = false;
    this.win = undefined;
  }

  stats(): MetricsSnapshot
  {
    return {
      fps: meanFps(this.intervals, this.filled),
      p95: percentile(this.intervals, this.filled, 0.95),
      samples: this.filled,
      source: "raf",
    };
  }

  private reset(): void
  {
    this.last = 0;
    this.index = 0;
    this.filled = 0;
  }

  private onVisibility = (): void =>
  {
    if (this.win?.document.visibilityState === "hidden") this.reset();
  };

  private tick = (): void =>
  {
    const win = this.win;
    if (!this.running || !win) return;
    try
    {
      const now = win.performance.now();
      if (this.last > 0)
      {
        const delta = now - this.last;
        if (delta > 0 && delta < MAX_FRAME_MS)
        {
          this.intervals[this.index] = delta;
          this.index = (this.index + 1) % RING;
          if (this.filled < RING) this.filled++;
        }
      }
      this.last = now;
      win.requestAnimationFrame(this.tick);
    }
    catch
    {
      this.running = false;
    }
  };
}

/** Assert-based self-check; run from devtools/CDP (`__HF_SELFTEST__`) or `#selftest`. */
export function selftest(): string[]
{
  const failures: string[] = [];
  const check = (name: string, ok: boolean): void =>
  {
    if (!ok) failures.push(name);
  };

  check("p95 nearest-rank", percentile(new Float32Array([10, 20, 30, 40, 100]), 5, 0.95) === 100);
  check("p50 nearest-rank", percentile(new Float32Array([10, 20, 30, 40, 100]), 5, 0.5) === 30);
  check("percentile empty", percentile(new Float32Array([1, 2]), 0, 0.95) === 0);
  check("mean fps", Math.abs(meanFps(new Float32Array([16, 16, 16, 16]), 4) - 62.5) < 1e-9);
  check("mean fps empty", meanFps(new Float32Array([16]), 0) === 0);
  check(
    "frame limit clamp",
    frameLimit("locked", 1) === 10 && frameLimit("locked", 5000) === 1000 && frameLimit("gsync", 144) === 0,
  );

  return failures;
}
