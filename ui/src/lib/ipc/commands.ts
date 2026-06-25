// Typed command wrappers — the only place the UI calls `invoke`. Every argument
// and return type comes from the ts-rs–generated `bindings/*` (UI_REFERENCE §6:
// the UI never hand-writes an API type). Argument records use camelCase keys;
// Tauri maps them onto the snake_case Rust parameters (`{ frameId }` → `frame_id`).
import { invoke } from "@tauri-apps/api/core";

import type { Readiness } from "../../bindings/Readiness";
import type { JobStats } from "../../bindings/JobStats";
import type { FrameDetail } from "../../bindings/FrameDetail";
import type { FrameMeta } from "../../bindings/FrameMeta";
import type { SearchQuery } from "../../bindings/SearchQuery";
import type { SearchHit } from "../../bindings/SearchHit";
import type { CaptureControl } from "../../bindings/CaptureControl";
import type { VisionTarget } from "../../bindings/VisionTarget";
import type { AskRequest } from "../../bindings/AskRequest";
import type { ReportRequest } from "../../bindings/ReportRequest";
import type { ReportResponse } from "../../bindings/ReportResponse";
import type { SetModelTier } from "../../bindings/SetModelTier";
import type { ModelLane } from "../../bindings/ModelLane";
import type { TimeRange } from "../../bindings/TimeRange";
import type { TimelineBucket } from "../../bindings/TimelineBucket";
import type { InsightsSummary } from "../../bindings/InsightsSummary";
import type { Settings } from "../../bindings/Settings";
import type { StorageStats } from "../../bindings/StorageStats";
import type { MonitorInfo } from "../../bindings/MonitorInfo";
import type { AppSuppression } from "../../bindings/AppSuppression";

/** Liveness probe for the IPC bridge. */
export const ping = (): Promise<string> => invoke<string>("ping");

/** Current subsystem readiness (`03 §7`). */
export const getReadiness = (): Promise<Readiness> => invoke<Readiness>("get_readiness");

/** Aggregate job-queue counts (`03 §7`). */
export const getJobStats = (): Promise<JobStats> => invoke<JobStats>("get_job_stats");

/** Storage footprint for the StatusRail. */
export const getStorageStats = (): Promise<StorageStats> =>
  invoke<StorageStats>("get_storage_stats");

/** Connected monitor metadata for Settings. */
export const getMonitors = (): Promise<MonitorInfo[]> => invoke<MonitorInfo[]>("get_monitors");

/** Device ids reported by llama.cpp `--list-devices`. */
export const listSidecarDevices = (): Promise<string[]> =>
  invoke<string[]>("list_sidecar_devices");

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

/** Cancel a streaming grounded answer by request id. */
export const cancelAsk = (requestId: string): Promise<void> =>
  invoke<void>("cancel_ask", { requestId });

/** Generate a recall report over a time range (`03 §8b`); progress streams via
 *  `report_progress` events, the report returns when complete. */
export const generateReport = (request: ReportRequest): Promise<ReportResponse> =>
  invoke<ReportResponse>("generate_report", { request });

/** Cancel an in-flight report by request id (stops at the next pass boundary). */
export const cancelReport = (requestId: string): Promise<void> =>
  invoke<void>("cancel_report", { requestId });

/** Change the active model tier for a lane (hot-applies on the next request). */
export const setModelTier = (request: SetModelTier): Promise<void> =>
  invoke<void>("set_model_tier", { request });

/** Eagerly load a lane's model into the sidecar so the next request is instant. */
export const loadModel = (lane: ModelLane): Promise<void> =>
  invoke<void>("load_model", { lane });

/** Unload the resident sidecar model now, freeing VRAM/RAM. */
export const unloadModel = (): Promise<void> => invoke<void>("unload_model");

/** Frame-count density buckets over `[start, end)` for the Scanline Timeline. */
export const getTimeline = (range: TimeRange, bucketCount: number): Promise<TimelineBucket[]> =>
  invoke<TimelineBucket[]>("get_timeline", { range, bucketCount });

/** Lightweight frame list over `[start, end)`, newest-first, capped at `limit`
 *  (timeline thumbnails, deck recents, moment neighbours). */
export const getFrames = (range: TimeRange, limit: number): Promise<FrameMeta[]> =>
  invoke<FrameMeta[]>("get_frames", { range, limit });

/** The frame whose capture time is nearest `at` (unix ms), or `null` if the DB has
 *  no frames — resolves a timeline scan-head position to a concrete frame. */
export const getNearestFrame = (at: number, range?: TimeRange): Promise<FrameMeta | null> =>
  invoke<FrameMeta | null>("get_nearest_frame", { at, range: range ?? null });

/** The captures bracketing `at` (unix ms): up to `limitEach` closest frames on each
 *  side within `±halfWindowMs`, ascending by time, excluding the anchor. Backs a
 *  Moment's prev/next + context strip (the adjacent frames, not the window's newest). */
export const getFrameContext = (
  at: number,
  halfWindowMs: number,
  limitEach: number,
): Promise<FrameMeta[]> =>
  invoke<FrameMeta[]>("get_frame_context", { at, halfWindowMs, limitEach });

/** Truthful activity aggregates over `[start, end)` for the Insights screen. */
export const getInsights = (range: TimeRange, bucketCount: number): Promise<InsightsSummary> =>
  invoke<InsightsSummary>("get_insights", { range, bucketCount });

/** Read the persisted user settings (missing keys fall back to defaults). */
export const getSettings = (): Promise<Settings> => invoke<Settings>("get_settings");

/** Persist user settings; tiers hot-apply, the rest on restart / next capture. */
export const setSettings = (settings: Settings): Promise<void> =>
  invoke<void>("set_settings", { settings });

/** Per-app text-filter suppression metric — the guardrail against silent
 *  over-suppression (PR3, `03 §3b`). */
export const getTextFilterStats = (): Promise<AppSuppression[]> =>
  invoke<AppSuppression[]>("get_text_filter_stats");
