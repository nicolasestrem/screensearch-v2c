// The pure state machine behind the `ask` flow, split out from the hook so it can
// be unit-tested without a DOM (UI_REFERENCE §6). `useAsk` owns the subscription,
// request-id scoping, and dispatch; this module just folds a streamed
// `answer_delta` into a stable view-model. The invariants worth pinning: a `done`
// after an `error` must not resurrect the stream, and citations dedupe in
// first-seen order.
import type { AnswerDelta } from "../../bindings/AnswerDelta";

export type AskPhase = "idle" | "streaming" | "done" | "error";

export interface AskState {
  phase: AskPhase;
  /** Accumulated chain-of-thought (shown collapsed); empty when not requested. */
  thinking: string;
  /** Accumulated answer prose (markdown). */
  answer: string;
  /** Source frame ids supplied to the model, in first-seen order, deduplicated. */
  citations: number[];
  error: string | null;
}

export const initialAskState: AskState = {
  phase: "idle",
  thinking: "",
  answer: "",
  citations: [],
  error: null,
};

export type AskAction = { type: "start" } | { type: "reset" } | { type: "delta"; delta: AnswerDelta };

export function askReducer(state: AskState, action: AskAction): AskState {
  switch (action.type) {
    case "reset":
      return initialAskState;
    case "start":
      return { ...initialAskState, phase: "streaming" };
    case "delta": {
      const d = action.delta;
      switch (d.type) {
        case "thinking":
          return { ...state, thinking: state.thinking + d.text };
        case "token":
          return { ...state, answer: state.answer + d.text };
        case "citation":
          // The current backend emits one id per frame included in the model prompt.
          // The UI labels these as checked context, not claim-level citations.
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
