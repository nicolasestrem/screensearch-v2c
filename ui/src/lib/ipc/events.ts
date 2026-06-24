// Typed event subscriptions. The payload map is the authoritative client view of
// what the backend emits (forward_events in src-tauri/src/lib.rs). Note: the
// `job_progress` event carries a bare `JobStats` (the kernel emits the inner value
// of KernelEvent::JobProgress), NOT the `JobProgress` wrapper binding.
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { CaptureTick } from "../../bindings/CaptureTick";
import type { Readiness } from "../../bindings/Readiness";
import type { JobStats } from "../../bindings/JobStats";
import type { JobCompleted } from "../../bindings/JobCompleted";
import type { SidecarStatus } from "../../bindings/SidecarStatus";
import type { ModelDownloadStatus } from "../../bindings/ModelDownloadStatus";
import type { AnswerEvent } from "../../bindings/AnswerEvent";
import type { Toast } from "../../bindings/Toast";

/** Map of backend event name → payload type. */
export interface AppEvents {
  capture_tick: CaptureTick;
  readiness_changed: Readiness;
  job_progress: JobStats;
  job_completed: JobCompleted;
  sidecar_status: SidecarStatus;
  model_download: ModelDownloadStatus;
  answer_delta: AnswerEvent;
  toast: Toast;
}

/**
 * Subscribe to a backend event with a typed payload. Returns the Tauri
 * `UnlistenFn` (call it to detach). Outside the Tauri shell `listen` rejects;
 * callers should treat a failed subscription as "no live events" (dev mode).
 */
export function listenTo<K extends keyof AppEvents>(
  event: K,
  handler: (payload: AppEvents[K]) => void,
): Promise<UnlistenFn> {
  return listen<AppEvents[K]>(event, (e) => handler(e.payload));
}
