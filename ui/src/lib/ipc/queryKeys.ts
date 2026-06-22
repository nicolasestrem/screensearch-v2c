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
  // `*Prefix` keys match every variant of a family for bulk invalidation (e.g.
  // a `capture_tick` invalidates all timeline ranges/bucket-counts at once; a
  // completed enrichment job invalidates all frame/search/insights variants).
  searchPrefix: ["search"] as const,
  search: (query: SearchQuery) => ["search", query] as const,
  timelinePrefix: ["timeline"] as const,
  timeline: (range: TimeRange, bucketCount: number) => ["timeline", range, bucketCount] as const,
  framePrefix: ["frame"] as const,
  frame: (frameId: number) => ["frame", frameId] as const,
  insightsPrefix: ["insights"] as const,
  insights: (range: TimeRange) => ["insights", range] as const,
};
