// Pure unit test for the `ask` state machine — the invariants that keep a streamed
// answer honest, with no DOM needed.
import { describe, it, expect } from "vitest";

import { askReducer, initialAskState, type AskState } from "../askReducer";

describe("askReducer", () => {
  it("start clears prior content and enters streaming", () => {
    const dirty: AskState = {
      phase: "done",
      thinking: "old",
      answer: "old answer",
      citations: [1, 2],
      error: "old error",
    };
    expect(askReducer(dirty, { type: "start" })).toEqual({
      ...initialAskState,
      phase: "streaming",
    });
  });

  it("accumulates thinking and answer tokens in order", () => {
    let s = askReducer(initialAskState, { type: "start" });
    s = askReducer(s, { type: "delta", delta: { type: "thinking", text: "rea" } });
    s = askReducer(s, { type: "delta", delta: { type: "thinking", text: "son" } });
    s = askReducer(s, { type: "delta", delta: { type: "token", text: "Hello " } });
    s = askReducer(s, { type: "delta", delta: { type: "token", text: "world" } });
    expect(s.thinking).toBe("reason");
    expect(s.answer).toBe("Hello world");
    expect(s.phase).toBe("streaming");
  });

  it("dedupes citations in first-seen order", () => {
    let s = askReducer(initialAskState, { type: "start" });
    s = askReducer(s, { type: "delta", delta: { type: "citation", frame_id: 7 } });
    s = askReducer(s, { type: "delta", delta: { type: "citation", frame_id: 3 } });
    s = askReducer(s, { type: "delta", delta: { type: "citation", frame_id: 7 } });
    expect(s.citations).toEqual([7, 3]);
  });

  it("does not resurrect a stream: done after error stays error", () => {
    let s = askReducer(initialAskState, { type: "start" });
    s = askReducer(s, { type: "delta", delta: { type: "error", message: "boom" } });
    expect(s.phase).toBe("error");
    s = askReducer(s, { type: "delta", delta: { type: "done" } });
    expect(s.phase).toBe("error");
    expect(s.error).toBe("boom");
  });

  it("reset returns to the initial idle state", () => {
    const s = askReducer(
      { phase: "streaming", thinking: "x", answer: "y", citations: [1], error: null },
      { type: "reset" },
    );
    expect(s).toEqual(initialAskState);
  });
});
