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
/** Debounce window for coalescing job_progress bursts (e.g. an enrichment backlog drain). */
const ENRICH_DEBOUNCE_MS = 1000;

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

    // job_progress fires on every job state change. Update the live queue counter
    // immediately, then debounce-invalidate the data a *completed* job changed:
    // vision_tag → frame detail + insights (tags / activity); embed_* → search
    // (the vector arm). The event carries only counts (no kind / frame id), so the
    // families are invalidated broadly — invalidateQueries refetches only the
    // *observed* queries and marks the rest stale, so an idle backlog drain stays
    // cheap. A surgical per-frame refresh would need a richer completion event
    // (07 #30). Timeline is intentionally excluded (capture density, not enrichment).
    let enrichTimer: ReturnType<typeof setTimeout> | undefined;
    track(
      listenTo("job_progress", (stats) => {
        qc.setQueryData(queryKeys.jobStats, stats);
        if (enrichTimer) clearTimeout(enrichTimer);
        enrichTimer = setTimeout(() => {
          qc.invalidateQueries({ queryKey: queryKeys.framePrefix });
          qc.invalidateQueries({ queryKey: queryKeys.searchPrefix });
          qc.invalidateQueries({ queryKey: queryKeys.insightsPrefix });
        }, ENRICH_DEBOUNCE_MS);
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
          qc.invalidateQueries({ queryKey: queryKeys.timelinePrefix });
          qc.invalidateQueries({ queryKey: queryKeys.insightsPrefix });
          // New frames change the newest-first lists (timeline thumbnails, deck
          // recents); refresh them alongside the density ribbon.
          qc.invalidateQueries({ queryKey: queryKeys.framesPrefix });
          qc.invalidateQueries({ queryKey: queryKeys.jobStats });
        }, TICK_DEBOUNCE_MS);
      }),
    );

    return () => {
      active = false;
      if (tickTimer) clearTimeout(tickTimer);
      if (enrichTimer) clearTimeout(enrichTimer);
      unlisteners.forEach((u) => u());
    };
  }, [qc]);
}
