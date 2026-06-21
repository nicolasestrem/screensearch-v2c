import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Readiness } from "./bindings/Readiness";

// P0 scaffold screen. It exercises the typed IPC end-to-end — `ping` and
// `get_readiness` — to prove the Tauri bridge + ts-rs bindings work. The full
// "Command Deck" UI (with TanStack Query, the full state matrix, and design
// tokens) replaces this in P5 per specs/UI_REFERENCE.md.

type Load<T> =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; data: T };

interface Probe {
  ping: string;
  readiness: Readiness;
}

export function App() {
  const [state, setState] = useState<Load<Probe>>({ status: "loading" });

  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const ping = await invoke<string>("ping");
        const readiness = await invoke<Readiness>("get_readiness");
        if (active) setState({ status: "ready", data: { ping, readiness } });
      } catch (e) {
        if (active) setState({ status: "error", message: String(e) });
      }
    })();
    return () => {
      active = false;
    };
  }, []);

  return (
    <main className="app">
      <h1>ScreenSearch</h1>
      <p className="subtitle">P0 scaffold — typed IPC smoke test</p>
      {state.status === "loading" && <p>Connecting to the kernel…</p>}
      {state.status === "error" && (
        <p className="error">
          IPC error: {state.message}
          <br />
          (Expected outside the Tauri shell — run <code>npm run dev</code> at the repo root.)
        </p>
      )}
      {state.status === "ready" && (
        <section>
          <p>
            Kernel says: <strong>{state.data.ping}</strong>
          </p>
          <ul className="readiness">
            {Object.entries(state.data.readiness).map(([component, status]) => (
              <li key={component}>
                <span>{component}</span>
                <span className={`status status-${status}`}>{status}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </main>
  );
}
