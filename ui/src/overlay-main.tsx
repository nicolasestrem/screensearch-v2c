import React from "react";
import ReactDOM from "react-dom/client";

import { Providers } from "./app/providers";
import { FlowOverlay } from "./overlay/FlowOverlay";
import { useLiveEvents } from "./lib/ipc/useLiveEvents";
import "./styles/globals.css";

export function OverlayRuntime() {
  useLiveEvents();
  return <FlowOverlay />;
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
