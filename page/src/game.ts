// The game runs in the `gameframe` iframe (same origin, D14). The page layer
// lives in the top frame; the engine runtime and the draw counters live inside
// the iframe, so everything here is a thin, fail-open accessor.

import type { EngineRuntime } from "./types";

export interface GameFrame
{
  element: HTMLIFrameElement;
  window: Window;
  document: Document;
}

/** Finds the official game frame. Undefined while it is missing or unreadable. */
export function findGameFrame(): GameFrame | undefined
{
  try
  {
    const frames = Array.from(document.querySelectorAll("iframe"));
    const element = frames.find((frame) => (frame.src || "").includes("game.html"));
    if (!element) return undefined;
    const contentWindow = element.contentWindow;
    const contentDocument = element.contentDocument;
    if (!contentWindow || !contentDocument) return undefined;
    return { element, window: contentWindow, document: contentDocument };
  }
  catch
  {
    return undefined; // cross-origin or detached: the HUD just waits
  }
}

export function engineOf(frame: GameFrame): EngineRuntime | undefined
{
  try
  {
    const engine = (frame.window as unknown as { __HAXFAST_ENGINE__?: EngineRuntime }).__HAXFAST_ENGINE__;
    return engine && typeof engine.set === "function" ? engine : undefined;
  }
  catch
  {
    return undefined;
  }
}
