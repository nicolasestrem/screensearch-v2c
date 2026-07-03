import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";

import { Providers } from "./app/providers";
import { FlowOverlay } from "./overlay/FlowOverlay";
import { MarkToast } from "./overlay/MarkToast";
import { listenTo } from "./lib/ipc/events";
import { useLiveEvents } from "./lib/ipc/useLiveEvents";
import type { MarkToast as MarkToastPayload } from "./bindings/MarkToast";
import "./styles/globals.css";

// The overlay window is shared between the Flow search/ask surface (PR5) and the
// mark-this-moment confirmation toast (PR6). A `mark_toast` event switches to the toast;
// `overlay_shown` (search summon) always wins back to the flow view; `overlay_hidden`
// resets. The two never render at once, so keyboard focus + five-state contracts stay
// scoped to whichever surface is up.
type View = { mode: "flow" } | { mode: "mark"; payload: MarkToastPayload };

export function OverlayRuntime() {
  useLiveEvents();
  const [view, setView] = useState<View>({ mode: "flow" });

  useEffect(() => {
    let active = true;
    const unlisteners: Array<() => void> = [];
    const track = (p: Promise<() => void>) =>
      p
        .then((u) => {
          if (active) unlisteners.push(u);
          else u();
        })
        .catch(() => {
          /* no Tauri runtime in browser dev */
        });

    track(
      listenTo("mark_toast", (payload) => setView({ mode: "mark", payload })),
    );
    track(listenTo("overlay_shown", () => setView({ mode: "flow" })));
    track(listenTo("overlay_hidden", () => setView({ mode: "flow" })));

    return () => {
      active = false;
      unlisteners.forEach((u) => u());
    };
  }, []);

  return view.mode === "mark" ? (
    <MarkToast
      payload={view.payload}
      onDone={() => setView({ mode: "flow" })}
    />
  ) : (
    <FlowOverlay />
  );
}

const rootEl = document.getElementById("root");
if (!rootEl) throw new Error("#root element not found");

ReactDOM.createRoot(rootEl).render(
  <React.StrictMode>
    <Providers>
      <OverlayRuntime />
    </Providers>
  </React.StrictMode>,
);
