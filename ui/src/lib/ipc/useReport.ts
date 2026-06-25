// The `generate_report` flow (UI_REFERENCE §4 Recall/reports). Unlike `ask` (which
// streams answer deltas), a report returns one value when complete; while it runs the
// backend emits request-scoped `report_progress` events ("Summarizing day 3 of 7") so
// the UI shows determinate progress instead of a dead spinner. Each run is scoped by a
// request id so a stale progress event for a superseded run is ignored, and cancel can
// stop the backend orchestrator between passes.
import { useCallback, useEffect, useReducer, useRef } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { listenTo } from "./events";
import * as cmd from "./commands";
import type { ReportRequest } from "../../bindings/ReportRequest";
import type { ReportResponse } from "../../bindings/ReportResponse";
import type { ReportProgress } from "../../bindings/ReportProgress";

export type ReportPhase = "idle" | "generating" | "done" | "error";

export interface ReportState {
  phase: ReportPhase;
  /** Latest progress tick while generating; `null` before the first one. */
  progress: { stage: string; done: number; total: number } | null;
  result: ReportResponse | null;
  error: string | null;
}

const initial: ReportState = { phase: "idle", progress: null, result: null, error: null };

type Action =
  | { type: "start" }
  | { type: "reset" }
  | { type: "progress"; progress: ReportProgress }
  | { type: "done"; result: ReportResponse }
  | { type: "error"; message: string };

function reducer(state: ReportState, action: Action): ReportState {
  switch (action.type) {
    case "reset":
      return initial;
    case "start":
      return { ...initial, phase: "generating" };
    case "progress":
      // A late progress tick after a terminal phase must not reopen the run.
      if (state.phase !== "generating") return state;
      return {
        ...state,
        progress: {
          stage: action.progress.stage,
          done: action.progress.done,
          total: action.progress.total,
        },
      };
    case "done":
      return { ...state, phase: "done", result: action.result, progress: null };
    case "error":
      return { ...state, phase: "error", error: action.message, progress: null };
  }
}

export interface UseReport extends ReportState {
  /** Generate a report; resolves when the run reaches a terminal phase. */
  generate: (request: Omit<ReportRequest, "request_id">) => Promise<void>;
  /** Ask the backend to stop the in-flight report (cooperative). */
  cancel: () => void;
  /** Clear back to idle. */
  reset: () => void;
}

export function useReport(): UseReport {
  const [state, dispatch] = useReducer(reducer, initial);
  const activeRequest = useRef<string | null>(null);

  // One persistent subscription; progress folds into the latest run only.
  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined;
    listenTo("report_progress", (payload) => {
      if (payload.request_id === activeRequest.current) {
        dispatch({ type: "progress", progress: payload });
      }
    })
      .then((u) => {
        if (active) unlisten = u;
        else u();
      })
      .catch(() => {
        /* no Tauri runtime (dev): no live progress */
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const generate = useCallback(async (request: Omit<ReportRequest, "request_id">) => {
    const requestId = `report-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
    activeRequest.current = requestId;
    dispatch({ type: "start" });
    try {
      const result = await cmd.generateReport({ ...request, request_id: requestId });
      if (activeRequest.current === requestId) dispatch({ type: "done", result });
    } catch (e) {
      if (activeRequest.current === requestId) dispatch({ type: "error", message: String(e) });
    } finally {
      if (activeRequest.current === requestId) activeRequest.current = null;
    }
  }, []);

  const cancel = useCallback(() => {
    const requestId = activeRequest.current;
    if (requestId) void cmd.cancelReport(requestId).catch(() => undefined);
    // The awaited generate() rejects with "report cancelled" → error phase; surface
    // idle instead so the builder is ready for another run.
    activeRequest.current = null;
    dispatch({ type: "reset" });
  }, []);

  const reset = useCallback(() => {
    activeRequest.current = null;
    dispatch({ type: "reset" });
  }, []);

  return { ...state, generate, cancel, reset };
}
