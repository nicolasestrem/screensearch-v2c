// AppShell — the persistent frame around every route (UI_REFERENCE §3): the
// StatusRail (top), ReadinessBanner (conditional), NavRail (left), the routed
// <Outlet>, the CommandPalette overlay, and the toast viewport. Mounts the single
// live-event subscription once, and the global ⌘K palette shortcut.
import { useEffect } from "react";
import { Outlet } from "react-router-dom";

import { StatusRail } from "./StatusRail";
import { NavRail } from "./NavRail";
import { ReadinessBanner } from "./ReadinessBanner";
import { CommandPalette } from "./CommandPalette";
import { ToastViewport } from "../primitives";
import { useLiveEvents } from "../../lib/ipc/useLiveEvents";
import { useUiStore } from "../../state/uiStore";

export function AppShell() {
  useLiveEvents();
  const togglePalette = useUiStore((s) => s.togglePalette);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        togglePalette();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [togglePalette]);

  return (
    <div className="flex flex-col h-full">
      <StatusRail />
      <ReadinessBanner />
      <div className="flex flex-1 min-h-0">
        <NavRail />
        <main className="flex-1 min-w-0 overflow-y-auto">
          <Outlet />
        </main>
      </div>
      <CommandPalette />
      <ToastViewport />
    </div>
  );
}
