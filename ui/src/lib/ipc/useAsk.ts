// The `ask` flow: a reducer that folds streamed `answer_delta` events into a
// stable view-model (UI_REFERENCE §6). One ask runs at a time; calling `ask`
// resets state and re-streams. The `answer_delta` subscription is owned here (not
// in useLiveEvents, which handles the StatusRail/timeline events).
import { useCallback, useEffect, useReducer, useRef } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { listenTo } from "./events";
import * as cmd from "./commands";
import type { AnswerDelta } from "../../bindings/AnswerDelta";
import type { AskRequest } from "../../bindings/AskRequest";

export type AskPhase = "idle" | "streaming" | "done" | "error";

export interface AskState {
  phase: AskPhase;
  /** Accumulated chain-of-thought (shown collapsed); empty when not requested. */
  thinking: string;
  /** Accumulated answer prose (markdown). */
  answer: string;
  /** Cited frame ids, in first-seen order, deduplicated. */
  citations: number[];
  error: string | null;
}

const initial: AskState = {
  phase: "idle",
  thinking: "",
  answer: "",
  citations: [],
  error: null,
};

type AskAction = { type: "start" } | { type: "reset" } | { type: "delta"; delta: AnswerDelta };

function reducer(state: AskState, action: AskAction): AskState {
  switch (action.type) {
    case "reset":
      return initial;
    case "start":
      return { ...initial, phase: "streaming" };
    case "delta": {
      const d = action.delta;
      switch (d.type) {
        case "thinking":
          return { ...state, thinking: state.thinking + d.text };
        case "token":
          return { ...state, answer: state.answer + d.text };
        case "citation":
          return state.citations.includes(d.frame_id)
            ? state
            : { ...state, citations: [...state.citations, d.frame_id] };
        case "done":
          // A `done` after an `error` must not resurrect the stream.
          return state.phase === "error" ? state : { ...state, phase: "done" };
        case "error":
          return { ...state, phase: "error", error: d.message };
      }
    }
  }
}

export interface UseAsk extends AskState {
  /** Start streaming an answer for `request`. */
  ask: (request: AskRequest) => Promise<void>;
  /** Clear back to idle. */
  reset: () => void;
}

export function useAsk(): UseAsk {
  const [state, dispatch] = useReducer(reducer, initial);

  // Synchronous in-flight guard. React state lags within an event tick, so two
  // ask() calls in the same tick would both pass a `state.phase` check and both
  // hit cmd.ask — and the single global `answer_delta` channel (no request id)
  // would then interleave the two streams. A ref set *before* dispatch blocks the
  // second call synchronously; it's released when the stream settles (below) or on
  // reset. A truly concurrent/cancellable ask needs a backend request-id (07 #28).
  const inFlight = useRef(false);

  // One persistent subscription for the lifetime of the hook; deltas always fold
  // into the latest state via the reducer.
  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined;
    listenTo("answer_delta", (delta) => dispatch({ type: "delta", delta }))
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
      inFlight.current = false;
    }
  }, [state.phase]);

  const ask = useCallback(async (request: AskRequest) => {
    // Block a concurrent ask synchronously: the ref reflects the in-flight state
    // immediately, unlike the reducer (which only updates on the next render).
    if (inFlight.current) return;
    inFlight.current = true;
    dispatch({ type: "start" });
    try {
      await cmd.ask(request);
    } catch (e) {
      // The command itself failed (e.g. sidecar not ready) before any delta.
      dispatch({ type: "delta", delta: { type: "error", message: String(e) } });
    }
  }, []);

  const reset = useCallback(() => {
    inFlight.current = false;
    dispatch({ type: "reset" });
  }, []);

  return { ...state, ask, reset };
}
