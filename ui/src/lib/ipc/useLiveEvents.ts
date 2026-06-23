// The single live-event subscription manager, mounted once in AppShell. It keeps
// server-state caches fresh from the backend event stream (UI_REFERENCE §6):
//   • readiness_changed → patch the readiness cache directly (no refetch)
//   • job_progress      → patch the jobStats cache directly
//   • job_completed     → surgical frame/search/insights invalidation
//   • sidecar_status    → patch the sidecarStatus cache + refresh readiness
//   • capture_tick      → high-frequency; debounce-invalidate timeline + insights
// `answer_delta` is intentionally NOT handled here — useAsk owns it.
import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { listenTo } from "./events";
import { queryKeys } from "./queryKeys";
import { toast as toastStore } from "../../state/toastStore";
import type { Readiness } from "../../bindings/Readiness";

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
        if (readiness.sidecar.status === "ready") {
          qc.invalidateQueries({ queryKey: queryKeys.sidecarDevices });
        }
      }),
    );

    track(
      listenTo("job_progress", (stats) => {
        qc.setQueryData(queryKeys.jobStats, stats);
      }),
    );

    let enrichTimer: ReturnType<typeof setTimeout> | undefined;
    track(
      listenTo("job_completed", (completed) => {
        qc.setQueryData(queryKeys.jobStats, completed.stats);
        qc.invalidateQueries({ queryKey: queryKeys.frame(completed.frame_id) });
        if (completed.kind === "embed_text" || completed.kind === "embed_image") {
          qc.invalidateQueries({ queryKey: queryKeys.searchPrefix });
        }
        if (completed.kind === "vision_tag") {
          if (enrichTimer) clearTimeout(enrichTimer);
          enrichTimer = setTimeout(() => {
            qc.invalidateQueries({ queryKey: queryKeys.insightsPrefix });
          }, ENRICH_DEBOUNCE_MS);
        }
      }),
    );

    track(
      listenTo("sidecar_status", (status) => {
        qc.setQueryData(queryKeys.sidecarStatus, status);
        // The kernel also re-emits readiness on a sidecar transition, but nudge a
        // refetch so the StatusRail stays truthful even if that event is missed.
        qc.invalidateQueries({ queryKey: queryKeys.readiness });
        qc.invalidateQueries({ queryKey: queryKeys.sidecarDevices });
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
          // recents) and a viewed moment's neighbour context; refresh them alongside
          // the density ribbon.
          qc.invalidateQueries({ queryKey: queryKeys.framesPrefix });
          qc.invalidateQueries({ queryKey: queryKeys.frameContextPrefix });
          qc.invalidateQueries({ queryKey: queryKeys.jobStats });
          qc.invalidateQueries({ queryKey: queryKeys.storageStats });
          const readiness = qc.getQueryData<Readiness>(queryKeys.readiness);
          if (readiness?.embed_model.status !== "ready") {
            qc.invalidateQueries({ queryKey: queryKeys.searchPrefix });
          }
        }, TICK_DEBOUNCE_MS);
      }),
    );

    track(
      listenTo("toast", (t) => {
        toastStore[t.level](t.message);
        if (t.message.toLowerCase().includes("retention")) {
          qc.invalidateQueries({ queryKey: queryKeys.storageStats });
          qc.invalidateQueries({ queryKey: queryKeys.framesPrefix });
          qc.invalidateQueries({ queryKey: queryKeys.timelinePrefix });
          qc.invalidateQueries({ queryKey: queryKeys.insightsPrefix });
        }
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
