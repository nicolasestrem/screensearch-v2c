// Session recap action (0.4.0 PR5): the existing report engine scoped to one
// stable session id. TanStack Mutation owns the returned server state; the only
// local state is transient request-scoped progress from the shared report event.
import { useCallback, useEffect, useRef, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { listenTo } from "./events";
import * as cmd from "./commands";
import { queryKeys } from "./queryKeys";
import type { ReportProgress } from "../../bindings/ReportProgress";
import type { SessionRecapRequest } from "../../bindings/SessionRecapRequest";

export interface SessionRecapProgress {
  stage: string;
  done: number;
  total: number;
}

export function useSessionRecap(sessionId: number | null) {
  const activeRequest = useRef<string | null>(null);
  const [progress, setProgress] = useState<SessionRecapProgress | null>(null);
  const recap = useMutation({
    mutationKey: queryKeys.sessionRecap(sessionId ?? -1),
    mutationFn: (request: SessionRecapRequest) => cmd.sessionRecap(request),
  });
  const { mutate } = recap;

  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined;
    listenTo("report_progress", (payload: ReportProgress) => {
      if (payload.request_id === activeRequest.current) {
        setProgress({
          stage: payload.stage,
          done: payload.done,
          total: payload.total,
        });
      }
    })
      .then((next) => {
        if (active) unlisten = next;
        else next();
      })
      .catch(() => {
        /* no Tauri runtime (browser dev): no live progress events */
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const generate = useCallback(() => {
    if (sessionId == null) return;
    const requestId = `session-recap-${sessionId}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
    activeRequest.current = requestId;
    setProgress(null);
    mutate(
      { session_id: sessionId, request_id: requestId },
      {
        onSettled: () => {
          if (activeRequest.current === requestId) {
            activeRequest.current = null;
            setProgress(null);
          }
        },
      },
    );
  }, [mutate, sessionId]);

  return {
    result: recap.data ?? null,
    error: recap.error,
    isGenerating: recap.isPending,
    isError: recap.isError,
    isSuccess: recap.isSuccess,
    progress,
    generate,
  };
}
