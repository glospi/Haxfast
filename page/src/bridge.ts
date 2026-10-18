// Page ↔ host bridge.
// F0: local WebSocket (127.0.0.1) with token; the host injects __HAXFAST_BOOT before the bundle.
// If CEF ever exposes nicer native IPC, ONLY this file changes.

type Boot = { wsUrl: string; token: string };

declare global {
  interface Window {
    __HAXFAST_BOOT?: Boot;
  }
}

type Pending = { resolve: (v: unknown) => void; reject: (e: unknown) => void };

let ws: WebSocket | undefined;
let seq = 0;
const pending = new Map<number, Pending>();

export const bridge = {
  /** Connects to the host. Returns false if the page runs outside HaxFast. */
  async connect(): Promise<boolean> {
    const boot = window.__HAXFAST_BOOT;
    if (!boot) {
      console.warn("[haxfast] no __HAXFAST_BOOT: running outside the client");
      return false;
    }
    return await new Promise((resolve) => {
      ws = new WebSocket(boot.wsUrl);
      ws.onopen = () => {
        // Mandatory first message: the host closes the connection without it.
        ws?.send(JSON.stringify({ method: "hello", params: { token: boot.token } }));
        resolve(true);
      };
      ws.onerror = () => resolve(false);
      ws.onmessage = (ev) => {
        let msg: { id?: number; result?: unknown; error?: string };
        try {
          msg = JSON.parse(String(ev.data));
        } catch {
          return; // invalid message: ignore it
        }
        if (typeof msg.id !== "number") return;
        const p = pending.get(msg.id);
        pending.delete(msg.id);
        if (msg.error !== undefined) p?.reject(new Error(msg.error));
        else p?.resolve(msg.result);
      };
    });
  },

  /** Call with response. With no host connected it resolves undefined so the page does not break. */
  call(method: string, params?: unknown): Promise<unknown> {
    return new Promise((resolve, reject) => {
      if (!ws || ws.readyState !== WebSocket.OPEN) {
        resolve(undefined);
        return;
      }
      const id = ++seq;
      pending.set(id, { resolve, reject });
      ws.send(JSON.stringify({ id, method, params }));
    });
  },

  /** Message without response (logs, metrics). */
  notify(method: string, params?: unknown): void {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ method, params }));
    }
  },
};

// Reachable from devtools (F12) to verify the gate by hand. No side effects.
(window as unknown as { __HF_BRIDGE__?: unknown }).__HF_BRIDGE__ = bridge;
