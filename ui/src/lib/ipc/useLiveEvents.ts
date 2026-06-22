// The single live-event subscription manager, mounted once in AppShell. It keeps
// server-state caches fresh from the backend event stream (UI_REFERENCE §6):
//   • readiness_changed → patch the readiness cache directly (no refetch)
//   • job_progress      → patch the jobStats cache directly
//   • sidecar_status    → patch the sidecarStatus cache + refresh readiness
//   • capture_tick      → high-frequency; debounce-invalidate timeline + insights
// `answer_delta` is intentionally NOT handled here — useAsk owns it.
import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { listenTo } from "./events";
import { queryKeys } from "./queryKeys";

/** Debounce window for coalescing capture_tick bursts into one invalidation. */
const TICK_DEBOUNCE_MS = 800;

export function useLiveEvents() {
  const qc = useQueryClient();

  useEffect(() => {
    let active = true;
    const unlisteners: UnlistenFn[] = [];
    const track = (p: Promise<UnlistenFn>) =>
      p
        .then((u) => {
          if (active) unlisteners.push(u);
          else u();
        })
        .catch(() => {
          /* no Tauri runtime (dev): no live events */
        });

    track(
      listenTo("readiness_changed", (readiness) => {
        qc.setQueryData(queryKeys.readiness, readiness);
      }),
    );

    track(
      listenTo("job_progress", (stats) => {
        qc.setQueryData(queryKeys.jobStats, stats);
      }),
    );

    track(
      listenTo("sidecar_status", (status) => {
        qc.setQueryData(queryKeys.sidecarStatus, status);
        // The kernel also re-emits readiness on a sidecar transition, but nudge a
        // refetch so the StatusRail stays truthful even if that event is missed.
        qc.invalidateQueries({ queryKey: queryKeys.readiness });
      }),
    );

    let tickTimer: ReturnType<typeof setTimeout> | undefined;
    track(
      listenTo("capture_tick", () => {
        if (tickTimer) clearTimeout(tickTimer);
        tickTimer = setTimeout(() => {
          qc.invalidateQueries({ queryKey: ["timeline"] });
          qc.invalidateQueries({ queryKey: ["insights"] });
          qc.invalidateQueries({ queryKey: queryKeys.jobStats });
        }, TICK_DEBOUNCE_MS);
      }),
    );

    return () => {
      active = false;
      if (tickTimer) clearTimeout(tickTimer);
      unlisteners.forEach((u) => u());
    };
  }, [qc]);
}
