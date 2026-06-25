// Read-side hooks. TanStack Query owns all server-state — one place for cache,
// loading, error, and refetch (UI_REFERENCE §6; no bespoke useEffect fetches).
import { useQuery, useQueryClient } from "@tanstack/react-query";

import * as cmd from "./commands";
import { queryKeys } from "./queryKeys";
import type { SearchQuery } from "../../bindings/SearchQuery";
import type { TimeRange } from "../../bindings/TimeRange";
import type { SidecarStatus } from "../../bindings/SidecarStatus";
import type { ModelDownloadStatus } from "../../bindings/ModelDownloadStatus";

/** Subsystem readiness; kept live by `readiness_changed` (see useLiveEvents). */
export function useReadiness() {
  return useQuery({ queryKey: queryKeys.readiness, queryFn: cmd.getReadiness });
}

/** Job-queue counts; kept live by `job_progress`. */
export function useJobStats() {
  return useQuery({ queryKey: queryKeys.jobStats, queryFn: cmd.getJobStats });
}

/** Storage footprint; refreshed by capture/retention events. */
export function useStorageStats() {
  return useQuery({ queryKey: queryKeys.storageStats, queryFn: cmd.getStorageStats });
}

/** Connected monitors for Settings. */
export function useMonitors() {
  return useQuery({ queryKey: queryKeys.monitors, queryFn: cmd.getMonitors });
}

/** llama.cpp device ids for advanced sidecar selection. */
export function useSidecarDevices(enabled = true) {
  return useQuery({
    queryKey: queryKeys.sidecarDevices,
    queryFn: cmd.listSidecarDevices,
    enabled,
    retry: 0,
  });
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

/**
 * Latest model-download progress (event-sourced, like {@link useSidecarStatus}). Populated
 * by the `model_download` event; `null` until a download starts. The queryFn preserves
 * whatever the event last wrote so a refetch never clobbers it.
 */
export function useModelDownload() {
  const qc = useQueryClient();
  return useQuery<ModelDownloadStatus | null>({
    queryKey: queryKeys.modelDownload,
    queryFn: () => qc.getQueryData<ModelDownloadStatus | null>(queryKeys.modelDownload) ?? null,
    staleTime: Infinity,
  });
}

/** Persisted settings. */
export function useSettings() {
  return useQuery({ queryKey: queryKeys.settings, queryFn: cmd.getSettings });
}

/** Per-app text-filter suppression rates (PR3 guardrail). */
export function useTextFilterStats(enabled = true) {
  return useQuery({
    queryKey: queryKeys.textFilterStats,
    queryFn: cmd.getTextFilterStats,
    enabled,
  });
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

/**
 * The captures bracketing a frame (closest on each side), for a Moment's prev/next
 * + context strip. Unlike `useFrames`, the result is anchored to `at` rather than the
 * window's newest frames, so the immediate neighbours are always present. Idle until
 * the anchor time is known (the owning `useFrame` has resolved).
 */
export function useFrameContext(
  at: number,
  halfWindowMs: number,
  limitEach: number,
  enabled = true,
) {
  return useQuery({
    queryKey: queryKeys.frameContext(at, halfWindowMs, limitEach),
    queryFn: () => cmd.getFrameContext(at, halfWindowMs, limitEach),
    enabled: enabled && halfWindowMs > 0 && limitEach > 0,
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
export function useInsights(range: TimeRange, bucketCount: number, enabled = true) {
  return useQuery({
    queryKey: queryKeys.insights(range, bucketCount),
    queryFn: () => cmd.getInsights(range, bucketCount),
    enabled: enabled && bucketCount > 0 && range.end > range.start,
  });
}
