// Shapes shared with the host (preferences.json) and with the engine runtime.
// The host validates and persists; the page only reads and sends back what it
// got. The defaults mirrored here are the same as `core::prefs` (D4/D20/D21).

export type GovernorMode = "gsync" | "locked" | "unlimited";
export type GpuBackend = "auto" | "gl" | "vulkan";

export interface VideoPrefs
{
  /** "gsync" | "locked" | "unlimited"; unknown values fall back to gsync. */
  governor: string;
  /** Draw cap for locked mode, in FPS. */
  fps_limit: number;
  show_hud: boolean;
  /** "auto" | "gl" | "vulkan"; applied at startup (Chromium flag). */
  gpu_backend: string;
}

export interface EngineToggles
{
  canvas_resize_cache: boolean;
  stadium_layer_cache: boolean;
  zero_input_tolerance: boolean;
  hidden_frame_skip: boolean;
  frame_governor: boolean;
}

export interface Preferences
{
  version: number;
  active_profile: string;
  profiles: unknown[];
  engine: EngineToggles;
  video: VideoPrefs;
}

export interface FrameStats
{
  fps: number;
  p95: number;
  samples: number;
  draws: number;
  limit: number;
}

/** The `__HAXFAST_ENGINE__` object injected into the game bundle (F1/F2). */
export interface EngineRuntime
{
  version: string;
  patches: string[];
  toggles: Record<string, boolean>;
  on: (name: string) => boolean;
  set: (name: string, value: boolean) => void;
  /** F2 frame-governor; absent when the engine runtime is old (fail-open). */
  setFrameLimit?: (fps: number) => void;
  frameStats?: () => FrameStats;
  /** Live draw cap (0 = none). */
  frameLimit?: number;
}

export const GOVERNOR_MODES: GovernorMode[] = ["gsync", "locked", "unlimited"];
export const GPU_BACKENDS: GpuBackend[] = ["auto", "gl", "vulkan"];

export const FPS_MIN = 10;
export const FPS_MAX = 1000;
export const FPS_DEFAULT = 144;

export function asGovernor(value: string): GovernorMode
{
  return (GOVERNOR_MODES as string[]).includes(value) ? (value as GovernorMode) : "gsync";
}

export function asBackend(value: string): GpuBackend
{
  return (GPU_BACKENDS as string[]).includes(value) ? (value as GpuBackend) : "auto";
}

export function clampFps(fps: number): number
{
  const value = Math.round(Number(fps));
  return Math.min(FPS_MAX, Math.max(FPS_MIN, Number.isFinite(value) ? value : FPS_DEFAULT));
}

/** Effective draw cap for a mode; 0 = no cap (mirrors `VideoPrefs::fps_limit` in Rust). */
export function frameLimit(mode: GovernorMode, fps: number): number
{
  return mode === "locked" ? clampFps(fps) : 0;
}
