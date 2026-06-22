// Central registry of TanStack Query keys. Prefix arrays are stable so live
// events can invalidate a whole family (e.g. `["timeline"]` matches every
// bucket-count/range variant) — see useLiveEvents.
import type { SearchQuery } from "../../bindings/SearchQuery";
import type { TimeRange } from "../../bindings/TimeRange";

export const queryKeys = {
  readiness: ["readiness"] as const,
  jobStats: ["jobStats"] as const,
  sidecarStatus: ["sidecarStatus"] as const,
  settings: ["settings"] as const,
  search: (query: SearchQuery) => ["search", query] as const,
  timeline: (range: TimeRange, bucketCount: number) => ["timeline", range, bucketCount] as const,
  frame: (frameId: number) => ["frame", frameId] as const,
  insights: (range: TimeRange) => ["insights", range] as const,
};
