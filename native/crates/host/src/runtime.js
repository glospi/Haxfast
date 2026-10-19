// HaxFast engine runtime — injected at the top of `game-min.js` (F1).
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
    if (state.tex && state.scale === scale && state.worldW === worldW && state.worldH === worldH &&
        T.Ya.x >= state.x0 + state.worldW * 0.25 &&
        T.Ya.x + state.worldW <= state.x0 + state.worldW * 1.75 &&
        T.Ya.y >= state.y0 + state.worldH * 0.25 &&
        T.Ya.y + state.worldH <= state.y0 + state.worldH * 1.75)
    {
      T.c.drawImage(state.tex, state.x0, state.y0, state.worldW * 2, state.worldH * 2);
      return;
    }
    // New region centered on the visible rect (snapped so panning is cheap).
    const regionX = Math.floor((T.Ya.x - worldW / 2) / BAKE_GRID) * BAKE_GRID;
    const regionY = Math.floor((T.Ya.y - worldH / 2) / BAKE_GRID) * BAKE_GRID;
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

  return E;
})();
