// Typed command wrappers — the only place the UI calls `invoke`. Every argument
// and return type comes from the ts-rs–generated `bindings/*` (UI_REFERENCE §6:
// the UI never hand-writes an API type). Argument records use camelCase keys;
// Tauri maps them onto the snake_case Rust parameters (`{ frameId }` → `frame_id`).
import { invoke } from "@tauri-apps/api/core";

import type { Readiness } from "../../bindings/Readiness";
import type { JobStats } from "../../bindings/JobStats";
import type { FrameDetail } from "../../bindings/FrameDetail";
import type { SearchQuery } from "../../bindings/SearchQuery";
import type { SearchHit } from "../../bindings/SearchHit";
import type { CaptureControl } from "../../bindings/CaptureControl";
import type { VisionTarget } from "../../bindings/VisionTarget";
import type { AskRequest } from "../../bindings/AskRequest";
import type { SetModelTier } from "../../bindings/SetModelTier";
import type { TimeRange } from "../../bindings/TimeRange";
import type { TimelineBucket } from "../../bindings/TimelineBucket";
import type { InsightsSummary } from "../../bindings/InsightsSummary";
import type { Settings } from "../../bindings/Settings";

/** Liveness probe for the IPC bridge. */
export const ping = (): Promise<string> => invoke<string>("ping");

/** Current subsystem readiness (`03 §7`). */
export const getReadiness = (): Promise<Readiness> => invoke<Readiness>("get_readiness");

/** Aggregate job-queue counts (`03 §7`). */
export const getJobStats = (): Promise<JobStats> => invoke<JobStats>("get_job_stats");

/** Full per-frame detail; `null` if the id is unknown. */
export const getFrame = (frameId: number): Promise<FrameDetail | null> =>
  invoke<FrameDetail | null>("get_frame", { frameId });

/** Hybrid search over OCR text + vectors, fused via RRF (`03 §7`). */
export const search = (query: SearchQuery): Promise<SearchHit[]> =>
  invoke<SearchHit[]>("search", { query });

/** Start/stop the always-on capture loop. */
export const captureControl = (control: CaptureControl): Promise<void> =>
  invoke<void>("capture_control", { control });

/** Enqueue deferred vision tagging; returns the number of jobs enqueued. */
export const enqueueVision = (target: VisionTarget): Promise<number> =>
  invoke<number>("enqueue_vision", { target });

/** Ask a grounded question; the answer streams back via `answer_delta` events. */
export const ask = (request: AskRequest): Promise<void> => invoke<void>("ask", { request });

/** Change the active model tier for a lane (hot-applies on the next request). */
export const setModelTier = (request: SetModelTier): Promise<void> =>
  invoke<void>("set_model_tier", { request });

/** Frame-count density buckets over `[start, end)` for the Scanline Timeline. */
export const getTimeline = (range: TimeRange, bucketCount: number): Promise<TimelineBucket[]> =>
  invoke<TimelineBucket[]>("get_timeline", { range, bucketCount });

/** Truthful activity aggregates over `[start, end)` for the Insights screen. */
export const getInsights = (range: TimeRange): Promise<InsightsSummary> =>
  invoke<InsightsSummary>("get_insights", { range });

/** Read the persisted user settings (missing keys fall back to defaults). */
export const getSettings = (): Promise<Settings> => invoke<Settings>("get_settings");

/** Persist user settings; tiers hot-apply, the rest on restart / next capture. */
export const setSettings = (settings: Settings): Promise<void> =>
  invoke<void>("set_settings", { settings });
