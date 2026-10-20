// F2 HUD: a minimal overlay in the top frame. It shows the frame metrics the
// engine reports for the game iframe, the governor state, the applied patches
// and the GPU backend Chromium actually selected (not the one requested).
//
// It lives in the top frame because the bridge and the future shell UI live
// there (F3); the game is touched only through its runtime object. The hotkey
// is provisional: F5 makes binds configurable (docs/PERFORMANCE.md).

import { asBackend, asGovernor, clampFps, FPS_MAX, FPS_MIN, GOVERNOR_MODES, GPU_BACKENDS } from "./types";
import type { EngineRuntime, EngineToggles, GovernorMode, Preferences, VideoPrefs } from "./types";

export interface HudSnapshot
{
  fps: number;
  p95: number;
  samples: number;
  source: "engine" | "raf";
  mode: GovernorMode;
  /** Requested cap when locked (0 otherwise). */
  limit: number;
  /** Cap actually applied to the runtime (0 = none / engine patch missing). */
  cap: number;
  native30: boolean;
}

export interface HudHooks
{
  engine: () => EngineRuntime | undefined;
  /** The video object is shared: mutate it and persist. */
  onVideo: (video: VideoPrefs) => void;
  onEngineToggle: (name: string, value: boolean) => void;
}

const HOTKEY = "F8";

const PATCHES: { key: keyof EngineToggles; name: string }[] = [
  { key: "zero_input_tolerance", name: "zero-input-tolerance" },
  { key: "canvas_resize_cache", name: "canvas-resize-cache" },
  { key: "stadium_layer_cache", name: "stadium-layer-cache" },
  { key: "hidden_frame_skip", name: "hidden-frame-skip" },
  { key: "frame_governor", name: "frame-governor" },
];

export class Hud
{
  readonly renderer: string;
  private root: HTMLDivElement;
  private line: HTMLDivElement;
  private panel: HTMLDivElement;
  // Built by buildPanel() before any render; the constructor owns the order.
  private governorSelect!: HTMLSelectElement;
  private limitInput!: HTMLInputElement;
  private gpuSelect!: HTMLSelectElement;
  private restart!: HTMLDivElement;
  private engineInputs = new Map<string, { input: HTMLInputElement; row: HTMLLabelElement }>();
  private startupVideo: VideoPrefs;

  constructor(private prefs: Preferences, private hooks: HudHooks)
  {
    this.startupVideo = { ...prefs.video };
    this.renderer = detectRenderer();

    this.root = document.createElement("div");
    this.root.id = "haxfast-hud";
    this.root.style.cssText =
      "position:fixed;right:10px;bottom:10px;z-index:2147483647;" +
      "font:12px/1.45 ui-monospace,SFMono-Regular,Menlo,monospace;color:#e6e6e6;" +
      "background:rgba(18,18,22,.86);border:1px solid rgba(255,255,255,.18);" +
      "border-radius:6px;padding:6px 8px;user-select:none;cursor:pointer;" +
      "white-space:nowrap;box-shadow:0 2px 10px rgba(0,0,0,.35)";

    this.line = document.createElement("div");
    this.line.textContent = "haxfast: waiting for the game frame…";
    this.root.append(this.line);

    this.panel = this.buildPanel();
    this.root.append(this.panel);

    this.root.addEventListener("click", () =>
    {
      this.panel.style.display = this.panel.style.display === "none" ? "block" : "none";
      this.renderRestart();
    });

    document.body.append(this.root);
  }

  /** F8: show/hide and persist. */
  toggle(): void
  {
    const visible = this.root.style.display !== "none";
    this.setVisible(!visible);
    this.prefs.video.show_hud = !visible;
    this.hooks.onVideo(this.prefs.video);
  }

  setVisible(visible: boolean): void
  {
    this.root.style.display = visible ? "block" : "none";
  }

  render(snapshot: HudSnapshot): void
  {
    const runtime = this.hooks.engine();
    const applied = new Set(runtime?.patches ?? []);
    const total = PATCHES.length;
    const count = PATCHES.filter((patch) => applied.has(patch.name) && runtime?.on(patch.name) === true).length;
    const hasSamples = snapshot.samples > 0;
    const approx = snapshot.source === "raf" && hasSamples ? "~" : "";
    const fps = hasSamples ? snapshot.fps.toFixed(1) : "—";
    const p95 = hasSamples ? snapshot.p95.toFixed(1) : "—";
    const mode = snapshot.mode === "locked"
      ? (snapshot.cap > 0 ? `locked ${snapshot.cap}` : "locked (no engine)")
      : snapshot.mode;

    const parts = [
      `${approx}${fps} fps`,
      `p95 ${p95} ms`,
      mode,
      runtime ? `${count}/${total}` : "engine —",
      backendShort(this.renderer),
    ];
    if (snapshot.native30) parts.push("native 30");
    this.line.textContent = parts.join(" · ");
    this.line.title = this.renderer;

    if (document.activeElement !== this.governorSelect)
    {
      this.governorSelect.value = snapshot.mode;
    }
    if (document.activeElement !== this.gpuSelect)
    {
      this.gpuSelect.value = asBackend(this.prefs.video.gpu_backend);
    }
    if (document.activeElement !== this.limitInput)
    {
      this.limitInput.disabled = snapshot.mode !== "locked";
      this.limitInput.value = String(clampFps(this.prefs.video.fps_limit));
      this.limitInput.style.opacity = snapshot.mode === "locked" ? "1" : "0.45";
    }

    for (const [name, entry] of this.engineInputs)
    {
      const missing = runtime !== undefined && !applied.has(name);
      if (document.activeElement !== entry.input) entry.input.disabled = missing;
      entry.row.style.opacity = missing ? "0.5" : "1";
      entry.row.title = missing ? "anchor not found this startup (fail-open)" : "";
    }

    this.renderRestart();
  }

  private buildPanel(): HTMLDivElement
  {
    const panel = document.createElement("div");
    panel.style.cssText =
      "margin-top:6px;padding-top:6px;border-top:1px solid rgba(255,255,255,.15);" +
      "display:none;width:280px;white-space:normal;cursor:default";
    panel.addEventListener("click", (event) => event.stopPropagation());

    this.governorSelect = selectOf(GOVERNOR_MODES.map((mode) => ({ value: mode, label: mode })));
    this.governorSelect.value = asGovernor(this.prefs.video.governor);
    this.governorSelect.addEventListener("change", () =>
    {
      this.prefs.video.governor = this.governorSelect.value;
      this.hooks.onVideo(this.prefs.video);
      this.renderRestart();
    });
    panel.append(rowOf("governor", this.governorSelect));

    this.limitInput = document.createElement("input");
    this.limitInput.type = "number";
    this.limitInput.min = String(FPS_MIN);
    this.limitInput.max = String(FPS_MAX);
    this.limitInput.step = "1";
    this.limitInput.value = String(clampFps(this.prefs.video.fps_limit));
    this.limitInput.style.cssText =
      "width:66px;background:#26262c;color:#eee;border:1px solid #444;border-radius:3px;font:inherit";
    this.limitInput.addEventListener("change", () =>
    {
      this.prefs.video.fps_limit = clampFps(Number(this.limitInput.value));
      this.limitInput.value = String(this.prefs.video.fps_limit);
      this.hooks.onVideo(this.prefs.video);
    });
    panel.append(rowOf("fps limit", this.limitInput));

    this.gpuSelect = selectOf(GPU_BACKENDS.map((backend) => ({ value: backend, label: backend })));
    this.gpuSelect.value = asBackend(this.prefs.video.gpu_backend);
    this.gpuSelect.addEventListener("change", () =>
    {
      this.prefs.video.gpu_backend = this.gpuSelect.value;
      this.hooks.onVideo(this.prefs.video);
      this.renderRestart();
    });
    panel.append(rowOf("gpu backend", this.gpuSelect));

    const renderer = document.createElement("div");
    renderer.textContent = this.renderer;
    renderer.title = this.renderer;
    renderer.style.cssText =
      "font-size:10px;color:#9a9aa4;overflow:hidden;text-overflow:ellipsis;" +
      "white-space:nowrap;margin:2px 0 4px";
    panel.append(renderer);

    this.restart = document.createElement("div");
    this.restart.style.cssText = "color:#ffcf6b;margin:4px 0;display:none";
    panel.append(this.restart);

    const patches = document.createElement("div");
    patches.style.cssText = "margin-top:4px";
    const heading = document.createElement("div");
    heading.textContent = "engine patches";
    heading.style.cssText = "color:#9a9aa4;margin-bottom:2px";
    patches.append(heading);
    for (const patch of PATCHES)
    {
      const row = document.createElement("label");
      row.style.cssText = "display:flex;align-items:center;gap:6px;cursor:pointer";
      const input = document.createElement("input");
      input.type = "checkbox";
      input.checked = this.prefs.engine[patch.key] !== false;
      input.addEventListener("change", () =>
      {
        this.prefs.engine[patch.key] = input.checked;
        this.hooks.onEngineToggle(patch.name, input.checked);
      });
      const text = document.createElement("span");
      text.textContent = patch.name;
      row.append(input, text);
      patches.append(row);
      this.engineInputs.set(patch.name, { input, row });
    }
    panel.append(patches);

    return panel;
  }

  private renderRestart(): void
  {
    const changed: string[] = [];
    if (this.prefs.video.governor !== this.startupVideo.governor) changed.push("governor");
    if (this.prefs.video.gpu_backend !== this.startupVideo.gpu_backend) changed.push("gpu");
    this.restart.style.display = changed.length > 0 ? "block" : "none";
    this.restart.textContent = changed.length > 0 ? `restart to apply: ${changed.join(", ")}` : "";
  }
}

/** Provisional F2 hotkey. F5 replaces this with configurable binds. */
export function installHotkey(hud: Hud, target: Document | undefined): void
{
  if (!target || bound.has(target)) return;
  bound.add(target);
  target.addEventListener("keydown", (event) =>
  {
    if (event.key !== HOTKEY || isTyping(event.target)) return;
    hud.toggle();
  }, true);
}

// Keyed by Document: the iframe WindowProxy survives navigations, the document
// does not, so a reload must install the listener on the new document.
const bound = new WeakSet<Document>();

function isTyping(target: EventTarget | null): boolean
{
  const element = target as HTMLElement | null;
  if (!element) return false;
  const tag = element.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || element.isContentEditable;
}

function rowOf(label: string, control: HTMLElement): HTMLDivElement
{
  const row = document.createElement("div");
  row.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:8px;margin:4px 0";
  const text = document.createElement("span");
  text.textContent = label;
  row.append(text, control);
  return row;
}

function selectOf(options: { value: string; label: string }[]): HTMLSelectElement
{
  const select = document.createElement("select");
  select.style.cssText =
    "background:#26262c;color:#eee;border:1px solid #444;border-radius:3px;font:inherit";
  for (const option of options)
  {
    const element = document.createElement("option");
    element.value = option.value;
    element.textContent = option.label;
    select.append(element);
  }
  return select;
}

/** The backend that actually ended up active; Chromium can silently fall back. */
function detectRenderer(): string
{
  try
  {
    const canvas = document.createElement("canvas");
    const gl = (canvas.getContext("webgl2") ?? canvas.getContext("webgl")) as
      | WebGLRenderingContext
      | WebGL2RenderingContext
      | null;
    if (!gl) return "no WebGL";
    const info = gl.getExtension("WEBGL_debug_renderer_info");
    const renderer = info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    return String(renderer);
  }
  catch (error)
  {
    return `unknown (${String(error)})`;
  }
}

function backendShort(renderer: string): string
{
  if (/vulkan/i.test(renderer)) return "Vulkan";
  if (/direct3d|d3d/i.test(renderer)) return "D3D";
  if (/swiftshader|software/i.test(renderer)) return "SW";
  if (/opengl|\bgl\b/i.test(renderer)) return "GL";
  return "GPU";
}
