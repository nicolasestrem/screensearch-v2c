import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Readiness } from "./bindings/Readiness";
import type { CaptureTick } from "./bindings/CaptureTick";
import type { CaptureControl } from "./bindings/CaptureControl";

// P2 screen: a minimal *live timeline*. It starts/stops the always-on capture loop
// (`capture_control`), shows live subsystem readiness, and appends a row per stored
// frame as `capture_tick` events arrive. The full "Command Deck" UI (TanStack
// Query, the full state matrix, design tokens, the Scanline-Timeline identity)
// replaces this in P5 per specs/UI_REFERENCE.md.

type Load<T> =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; data: T };

const MAX_TICKS = 50;

export function App() {
  const [probe, setProbe] = useState<Load<{ ping: string }>>({ status: "loading" });
  const [readiness, setReadiness] = useState<Readiness | null>(null);
  const [ticks, setTicks] = useState<CaptureTick[]>([]);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const unlisteners: UnlistenFn[] = [];

    (async () => {
      try {
        const [ping, current] = await Promise.all([
          invoke<string>("ping"),
          invoke<Readiness>("get_readiness"),
        ]);
        if (!active) return;
        setProbe({ status: "ready", data: { ping } });
        setReadiness(current);
      } catch (e) {
        if (active) setProbe({ status: "error", message: String(e) });
      }

      // Live updates: a row per stored frame, and readiness transitions.
      const onTick = await listen<CaptureTick>("capture_tick", (ev) =>
        setTicks((prev) => [ev.payload, ...prev].slice(0, MAX_TICKS)),
      );
      const onReadiness = await listen<Readiness>("readiness_changed", (ev) =>
        setReadiness(ev.payload),
      );
      if (active) {
        unlisteners.push(onTick, onReadiness);
      } else {
        onTick();
        onReadiness();
      }
    })();

    return () => {
      active = false;
      unlisteners.forEach((u) => u());
    };
  }, []);

  const captureStatus = readiness?.capture.status ?? "unknown";
  const capturing = captureStatus === "ready";
  const starting = captureStatus === "initializing";

  async function setCapture(control: CaptureControl) {
    setBusy(true);
    setActionError(null);
    try {
      await invoke("capture_control", { control });
    } catch (e) {
      setActionError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="app">
      <h1>ScreenSearch</h1>
      <p className="subtitle">P2 — live capture</p>

      {probe.status === "loading" && <p>Connecting to the kernel…</p>}

      {probe.status === "error" && (
        <p className="error">
          IPC error: {probe.message}
          <br />
          (Expected outside the Tauri shell — run <code>npm run dev</code> at the repo root.)
        </p>
      )}

      {probe.status === "ready" && (
        <>
          {readiness && (
            <ul className="readiness">
              {Object.entries(readiness).map(([component, cr]) => (
                <li key={component}>
                  <span>{component}</span>
                  <span className={`status status-${cr.status}`}>
                    {cr.status}
                    {cr.detail ? ` — ${cr.detail}` : ""}
                  </span>
                </li>
              ))}
            </ul>
          )}

          <section className="controls">
            <button
              type="button"
              onClick={() => setCapture("start")}
              disabled={busy || capturing || starting}
            >
              {starting ? "Starting…" : "Start capture"}
            </button>
            <button
              type="button"
              onClick={() => setCapture("stop")}
              disabled={busy || !capturing}
            >
              Stop capture
            </button>
            <span className={`status status-${captureStatus}`}>{captureStatus}</span>
          </section>

          {actionError && <p className="error">Capture error: {actionError}</p>}

          <section className="timeline">
            <h2>
              Live timeline <span className="muted">({ticks.length})</span>
            </h2>
            {ticks.length === 0 ? (
              <p className="muted">
                {capturing
                  ? "Waiting for the first changed frame…"
                  : "No frames yet — start capture to record your screen."}
              </p>
            ) : (
              <ul className="ticks">
                {ticks.map((t) => (
                  <li key={`${t.frame_id}`}>
                    <span className="tick-id">#{t.frame_id}</span>
                    <span>{new Date(t.captured_at).toLocaleTimeString()}</span>
                    <span className="muted">monitor {t.monitor_index}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </>
      )}
    </main>
  );
}
