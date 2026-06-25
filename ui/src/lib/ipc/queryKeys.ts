// Central registry of TanStack Query keys. Prefix arrays are stable so live
// events can invalidate a whole family (e.g. `["timeline"]` matches every
// bucket-count/range variant) — see useLiveEvents.
import type { SearchQuery } from "../../bindings/SearchQuery";
import type { TimeRange } from "../../bindings/TimeRange";

export const queryKeys = {
  readiness: ["readiness"] as const,
  jobStats: ["jobStats"] as const,
  storageStats: ["storageStats"] as const,
  monitors: ["monitors"] as const,
  sidecarDevices: ["sidecarDevices"] as const,
  sidecarStatus: ["sidecarStatus"] as const,
  modelDownload: ["modelDownload"] as const,
  settings: ["settings"] as const,
  textFilterStats: ["textFilterStats"] as const,
  // `*Prefix` keys match every variant of a family for bulk invalidation (e.g.
  // a `capture_tick` invalidates all timeline ranges/bucket-counts at once; a
  // completed enrichment job invalidates all frame/search/insights variants).
  searchPrefix: ["search"] as const,
  search: (query: SearchQuery) => ["search", query] as const,
  timelinePrefix: ["timeline"] as const,
  timeline: (range: TimeRange, bucketCount: number) => ["timeline", range, bucketCount] as const,
  framePrefix: ["frame"] as const,
  frame: (frameId: number) => ["frame", frameId] as const,
  // Frame *lists* (FrameMeta) — distinct from the singular `frame` detail above.
  // A new capture (`capture_tick`) invalidates every range/limit variant at once.
  framesPrefix: ["frames"] as const,
  frames: (range: TimeRange, limit: number) => ["frames", range, limit] as const,
  // A frame's neighbour context (closest captures on each side). A new capture
  // (`capture_tick`) can add a neighbour, so this family is invalidated alongside
  // the frame lists above.
  frameContextPrefix: ["frameContext"] as const,
  frameContext: (at: number, halfWindowMs: number, limitEach: number) =>
    ["frameContext", at, halfWindowMs, limitEach] as const,
  insightsPrefix: ["insights"] as const,
  insights: (range: TimeRange, bucketCount: number) => ["insights", range, bucketCount] as const,
};
