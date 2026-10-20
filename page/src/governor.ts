// F2 governor. The mode is a Chromium startup choice (gsync has no flags,
// locked/unlimited disable vsync + frame-rate limit); the draw cap itself is
// applied live to the engine runtime. The page never wraps requestAnimationFrame:
// simulation, network and input stay at the monitor cadence (D22).

import { frameLimit } from "./types";
import type { EngineRuntime, GovernorMode } from "./types";

export type Log = (msg: string, data?: unknown) => void;

export function applyGovernor(
  engine: EngineRuntime | undefined,
  mode: GovernorMode,
  fps: number,
  log: Log,
): void
{
  if (!engine) return;
  if (typeof engine.setFrameLimit !== "function")
  {
    if (mode === "locked") log("governor: engine patch missing, locked has no effect (fail-open)");
    return;
  }
  engine.setFrameLimit(frameLimit(mode, fps));
}
