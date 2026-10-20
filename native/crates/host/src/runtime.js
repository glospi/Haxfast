// HaxFast engine runtime — injected at the top of `game-min.js` (F1/F2).
// Every patched site routes through this object; `on()` is read live, so
// `__HAXFAST_ENGINE__.set(name, false)` turns a patch off without a reload.
// Project rule: any failure here must degrade to the vanilla rendering
// (fail-open: never break the ability to play).

;
globalThis.__HAXFAST_ENGINE__ = (function ()
{
  const E = {
    version: "__HF_VERSION__",
    patches: __HF_PATCHES__,
    toggles: __HF_TOGGLES__,
    on: (name) => E.toggles[name] !== false,
    set: (name, value) =>
    {
      E.toggles[name] = value !== false;
    },
  };

  // hidden-frame-skip: the live-view loop `sf()` keeps scheduling rAF but skips
  // the whole per-frame chain while the page is hidden. Resuming is exactly the
  // vanilla tab-hidden behavior (one long frame delta), so nothing new is added.
  E.shouldRender = function ()
  {
    return !E.on("hidden-frame-skip") || document.visibilityState === "visible";
  };

  // canvas-resize-cache: cache the canvas rect with a ResizeObserver (it is the
  // only per-frame cost of the vanilla resize method; the backing-store write
  // stays guarded and devicePixelRatio stays live at the call site).
  const rects = new WeakMap();
  E.rect = function (canvas)
  {
    if (!E.on("canvas-resize-cache")) return canvas.getBoundingClientRect();
    let entry = rects.get(canvas);
    if (!entry)
    {
      entry = { rect: canvas.getBoundingClientRect(), live: true };
      rects.set(canvas, entry);
      try
      {
        if (window.ResizeObserver)
        {
          const observer = new window.ResizeObserver(() =>
          {
            entry.rect = canvas.getBoundingClientRect();
          });
          observer.observe(canvas);
          entry.live = false;
        }
      }
      catch (e)
      {
        // This canvas keeps measuring live (fail-open).
      }
    }
    return entry.live ? canvas.getBoundingClientRect() : entry.rect;
  };

  // stadium-layer-cache: bake the stadium background + layers (Dr + Cr) into an
  // offscreen texture covering a 2x-viewport world region around the camera.
  // Inside the region a frame is a single drawImage into the world transform the
  // bundle set up just before the call site; panning less than half a viewport
  // re-bakes. Over budget or any error → vanilla live drawing for that frame.
  const stadiumState = new WeakMap();
  const BAKE_GRID = 64;            // world units; region re-anchor granularity
  const BAKE_MAX_PIXELS = 16000000; // texture budget (shortcut: blur if crossed)

  E.stadium = function (T, zoom, stadium)
  {
    if (!E.on("stadium-layer-cache") || zoom <= 0)
    {
      T.Dr(stadium);
      T.Cr(stadium);
      return;
    }
    let state = stadiumState.get(T);
    if (!state)
    {
      state = {};
      stadiumState.set(T, state);
    }
    const canvas = T.oa;
    const worldW = canvas.width / zoom; // visible world size at this zoom
    const worldH = canvas.height / zoom;
    // Texture at screen scale (pixel-perfect blit); shrink only over budget.
    let scale = zoom;
    if (4 * canvas.width * canvas.height > BAKE_MAX_PIXELS)
    {
      const squeezed = zoom * Math.sqrt(BAKE_MAX_PIXELS / (4 * canvas.width * canvas.height));
      scale = squeezed < zoom * 0.25 ? zoom * 0.25 : squeezed;
    }
    if (state.tex && state.scale === scale && state.worldW === worldW && state.worldH === worldH)
    {
      // Reuse only while the visible world rect (the transform includes the HUD
      // offset) stays inside the baked region; otherwise re-bake. Anything less
      // than full containment leaves stale pixels, because this blit replaced
      // the full-canvas clear of Dr.
      const m = T.c.getTransform();
      if (m.a > 0)
      {
        const left = -m.e / m.a;
        const right = (canvas.width - m.e) / m.a;
        const top = -m.f / m.a;
        const bottom = (canvas.height - m.f) / m.a;
        if (left >= state.x0 && right <= state.x0 + state.worldW * 2 &&
            top >= state.y0 && bottom <= state.y0 + state.worldH * 2)
        {
          T.c.drawImage(state.tex, state.x0, state.y0, state.worldW * 2, state.worldH * 2);
          return;
        }
      }
    }
    // New region centered on the camera (snapped so panning is cheap).
    const regionX = Math.floor((T.Ya.x - worldW) / BAKE_GRID) * BAKE_GRID;
    const regionY = Math.floor((T.Ya.y - worldH) / BAKE_GRID) * BAKE_GRID;
    const texW = Math.max(1, Math.round(worldW * 2 * scale));
    const texH = Math.max(1, Math.round(worldH * 2 * scale));
    const tex = document.createElement("canvas");
    tex.width = texW;
    tex.height = texH;
    const texCtx = tex.getContext("2d", { alpha: false });
    if (!texCtx)
    {
      T.Dr(stadium);
      T.Cr(stadium);
      return;
    }
    const mainCtx = T.c;
    const realCanvas = T.oa;
    try
    {
      // Dr type 1 fills the full canvas in screen pixels after resetTransform:
      // the fake canvas size gives that fill the texture extents. Patterns
      // (tp/Fo/Do) were created on the main context and are used cross-context.
      T.oa = { width: texW, height: texH };
      T.c = texCtx;
      texCtx.setTransform(scale, 0, 0, scale, -regionX * scale, -regionY * scale);
      texCtx.lineWidth = 3; // the vanilla code sets this right before Dr
      T.Dr(stadium);
      T.Cr(stadium);
    }
    catch (e)
    {
      T.oa = realCanvas;
      T.c = mainCtx;
      T.Dr(stadium);
      T.Cr(stadium);
      return;
    }
    T.oa = realCanvas;
    T.c = mainCtx;
    state.tex = tex;
    state.x0 = regionX;
    state.y0 = regionY;
    state.scale = scale;
    state.worldW = worldW;
    state.worldH = worldH;
    T.c.drawImage(tex, regionX, regionY, worldW * 2, worldH * 2);
  };

  // frame-governor: the engine patch calls skipDraw() from the live-view draw
  // guard, so the cap skips the draw only (simulation, network and input keep
  // the monitor cadence). frameStats() reports the draws actually made; the HUD
  // reads them. With the toggle off or no cap this is vanilla + bookkeeping.
  const DRAW_RING = 256;
  const DRAW_MAX_MS = 250; // tab resumes and breakpoints are not frame time
  const drawIntervals = new Float32Array(DRAW_RING);
  let drawIndex = 0;
  let drawFilled = 0;
  let drawCount = 0;
  let lastDraw = 0;

  const resetDrawMetrics = function ()
  {
    drawIndex = 0;
    drawFilled = 0;
    drawCount = 0;
    lastDraw = 0;
  };

  E.frameLimit = 0;
  E.setFrameLimit = function (fps)
  {
    const value = Number(fps);
    const next = Number.isFinite(value) && value > 0 ? value : 0;
    if (next !== E.frameLimit)
    {
      E.frameLimit = next;
      resetDrawMetrics();
    }
  };

  E.skipDraw = function (now, nativeBlocked)
  {
    try
    {
      if (!E.on("frame-governor")) return nativeBlocked;
      if (nativeBlocked) return true;
      const interval = E.frameLimit > 0 ? 1000 / E.frameLimit : 0;
      // 10% early tolerance: without it a 30 fps cap on a 60 Hz display lands on
      // 20/30 fps alternately because of timestamp jitter around the interval.
      if (interval > 0 && now - lastDraw < interval * 0.9) return true;
      if (lastDraw > 0)
      {
        const delta = now - lastDraw;
        if (delta > 0 && delta < DRAW_MAX_MS)
        {
          drawIntervals[drawIndex] = delta;
          drawIndex = (drawIndex + 1) % DRAW_RING;
          if (drawFilled < DRAW_RING) drawFilled++;
        }
      }
      lastDraw = now;
      drawCount++;
      return false;
    }
    catch (e)
    {
      return nativeBlocked; // fail-open: the vanilla decision
    }
  };

  E.frameStats = function ()
  {
    try
    {
      // The replay/menu loop does not call skipDraw: stale counters would
      // otherwise freeze the HUD.
      const stale = lastDraw > 0 && performance.now() - lastDraw > 500;
      let sum = 0;
      for (let i = 0; i < drawFilled; i++) sum += drawIntervals[i];
      const mean = drawFilled > 0 ? sum / drawFilled : 0;
      let p95 = 0;
      if (drawFilled > 0)
      {
        const sorted = Array.from(drawIntervals.subarray(0, drawFilled)).sort(function (a, b)
        {
          return a - b;
        });
        const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(0.95 * sorted.length) - 1));
        p95 = sorted[index];
      }
      return {
        fps: !stale && mean > 0 ? 1000 / mean : 0,
        p95: stale ? 0 : p95,
        samples: stale ? 0 : drawFilled,
        draws: drawCount,
        limit: E.frameLimit,
      };
    }
    catch (e)
    {
      return { fps: 0, p95: 0, samples: 0, draws: drawCount, limit: E.frameLimit };
    }
  };

  // Hidden pages stop drawing while rAF keeps its schedule; reset so the
  // resume delta does not poison FPS or p95.
  document.addEventListener("visibilitychange", function ()
  {
    if (document.visibilityState === "hidden") resetDrawMetrics();
  });

  return E;
})();
