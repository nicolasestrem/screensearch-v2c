// The `ask` flow hook: owns the `answer_delta` subscription and request-id scoping,
// and folds deltas into a stable view-model via the pure `askReducer` (UI_REFERENCE
// §6). Each stream is scoped by a request id so superseded deltas are ignored and
// reset can cancel the backend provider task.
import { useCallback, useEffect, useReducer, useRef } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { listenTo } from "./events";
import * as cmd from "./commands";
import { askReducer, initialAskState, type AskState } from "./askReducer";
import type { AskRequest } from "../../bindings/AskRequest";

// Re-exported so existing consumers (e.g. AnswerStream) keep importing from here.
export type { AskPhase, AskState } from "./askReducer";

export interface UseAsk extends AskState {
  /** Start streaming an answer for `request`. */
  ask: (request: Omit<AskRequest, "request_id">) => Promise<void>;
  /** Clear back to idle. */
  reset: () => void;
}

export function useAsk(): UseAsk {
  const [state, dispatch] = useReducer(askReducer, initialAskState);
  const activeRequest = useRef<string | null>(null);

  // One persistent subscription for the lifetime of the hook; deltas always fold
  // into the latest state via the reducer.
  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined;
    listenTo("answer_delta", (event) => {
      if (event.request_id === activeRequest.current) {
        dispatch({ type: "delta", delta: event.delta });
      }
    })
      .then((u) => {
        if (active) unlisten = u;
        else u();
      })
      .catch(() => {
        /* no Tauri runtime (dev): no live deltas */
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  // Release the in-flight guard once the stream reaches a terminal phase, so the
  // next ask() can start. cmd.ask resolves immediately (the answer arrives later
  // as deltas), so the guard can't key off the awaited promise.
  useEffect(() => {
    if (state.phase === "done" || state.phase === "error") {
      activeRequest.current = null;
    }
  }, [state.phase]);

  const ask = useCallback(async (request: Omit<AskRequest, "request_id">) => {
    if (activeRequest.current) {
      await cmd.cancelAsk(activeRequest.current).catch(() => undefined);
    }
    const requestId = `ask-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
    activeRequest.current = requestId;
    dispatch({ type: "start" });
    try {
      await cmd.ask({ ...request, request_id: requestId });
    } catch (e) {
      // The command itself failed (e.g. sidecar not ready) before any delta.
      dispatch({ type: "delta", delta: { type: "error", message: String(e) } });
    }
  }, []);

  const reset = useCallback(() => {
    const requestId = activeRequest.current;
    if (requestId) {
      void cmd.cancelAsk(requestId).catch(() => undefined);
    }
    activeRequest.current = null;
    dispatch({ type: "reset" });
  }, []);

  return { ...state, ask, reset };
}
