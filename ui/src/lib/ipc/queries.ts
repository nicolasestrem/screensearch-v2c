// Read-side hooks. TanStack Query owns all server-state — one place for cache,
// loading, error, and refetch (UI_REFERENCE §6; no bespoke useEffect fetches).
import { useQuery, useQueryClient } from "@tanstack/react-query";

import * as cmd from "./commands";
import { queryKeys } from "./queryKeys";
import type { SearchQuery } from "../../bindings/SearchQuery";
import type { TimeRange } from "../../bindings/TimeRange";
import type { SidecarStatus } from "../../bindings/SidecarStatus";

/** Subsystem readiness; kept live by `readiness_changed` (see useLiveEvents). */
export function useReadiness() {
  return useQuery({ queryKey: queryKeys.readiness, queryFn: cmd.getReadiness });
}

/** Job-queue counts; kept live by `job_progress`. */
export function useJobStats() {
  return useQuery({ queryKey: queryKeys.jobStats, queryFn: cmd.getJobStats });
}

/**
 * Latest sidecar lifecycle status. There is no fetch command for this — it is an
 * event-sourced cache entry populated by `sidecar_status` (the queryFn just
 * preserves whatever the event last wrote, so a refetch never clobbers it).
 */
export function useSidecarStatus() {
  const qc = useQueryClient();
  return useQuery<SidecarStatus | null>({
    queryKey: queryKeys.sidecarStatus,
    queryFn: () => qc.getQueryData<SidecarStatus | null>(queryKeys.sidecarStatus) ?? null,
    staleTime: Infinity,
  });
}

/** Persisted settings. */
export function useSettings() {
  return useQuery({ queryKey: queryKeys.settings, queryFn: cmd.getSettings });
}

/** Hybrid search; idle until there is a non-empty query (no empty-string calls). */
export function useSearch(query: SearchQuery, enabled = true) {
  return useQuery({
    queryKey: queryKeys.search(query),
    queryFn: () => cmd.search(query),
    enabled: enabled && query.text.trim().length > 0,
  });
}

/** Timeline density buckets; invalidated (debounced) by `capture_tick`. */
export function useTimeline(range: TimeRange, bucketCount: number, enabled = true) {
  return useQuery({
    queryKey: queryKeys.timeline(range, bucketCount),
    queryFn: () => cmd.getTimeline(range, bucketCount),
    enabled: enabled && bucketCount > 0 && range.end > range.start,
  });
}

/**
 * Lightweight frame list over a window, newest-first (timeline thumbnails, deck
 * recents). Invalidated (debounced) by `capture_tick` as new frames land. Idle for
 * an empty/invalid window so we never ask for the whole table.
 */
export function useFrames(range: TimeRange, limit: number, enabled = true) {
  return useQuery({
    queryKey: queryKeys.frames(range, limit),
    queryFn: () => cmd.getFrames(range, limit),
    enabled: enabled && limit > 0 && range.end > range.start,
  });
}

/** One frame's full detail; idle until a frame id is selected. */
export function useFrame(frameId: number | null) {
  return useQuery({
    queryKey: queryKeys.frame(frameId ?? -1),
    queryFn: () => cmd.getFrame(frameId as number),
    enabled: frameId != null,
  });
}

/** Activity aggregates for the Insights screen. */
export function useInsights(range: TimeRange, enabled = true) {
  return useQuery({
    queryKey: queryKeys.insights(range),
    queryFn: () => cmd.getInsights(range),
    enabled: enabled && range.end > range.start,
  });
}
