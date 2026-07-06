// AnswerStream — renders the streamed `ask` answer (UI_REFERENCE §4 Recall/ask).
// Folds the useAsk view-model into: a collapsible *thinking* trace (native
// <details>, so keyboard + a11y come free), the answer prose (markdown via
// react-markdown + GFM, themed with the `prose-deck` typography tokens), and the
// reviewed source-frame context as clickable tiles that open each source moment.
// react-markdown is imported only here, so it ships in the /recall route chunk (§8).
// The idle phase renders nothing — the Recall screen owns the empty "ask a question"
// invitation.
import { useEffect, useRef, useState } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

import type { AskPhase } from "../../lib/ipc/useAsk";
import { CitationTile } from "./CitationTile";
import { ErrorState } from "../primitives";
import { handleExternalLinkClick } from "../../lib/openExternal";

/** Nearest scrollable ancestor of `el` (the shell `<main>` in the Recall route, the
 *  overlay's own pane in the Flow overlay) — the element to follow while the reasoning
 *  trace streams, now that the trace grows inline instead of scrolling itself (D9, #59). */
function nearestScrollable(el: HTMLElement): HTMLElement | null {
  for (let n = el.parentElement; n; n = n.parentElement) {
    const oy = getComputedStyle(n).overflowY;
    if ((oy === "auto" || oy === "scroll") && n.scrollHeight > n.clientHeight) {
      return n;
    }
  }
  return null;
}

export interface AnswerStreamProps {
  phase: AskPhase;
  thinking: string;
  answer: string;
  /** Frame ids supplied to the answer model as reviewed context. */
  citations: number[];
  error: string | null;
  /** Re-run the last question (shown on error). */
  onRetry?: () => void;
  /** Router-free opener used by both the main Recall route and the Flow overlay. */
  onOpenFrame: (frameId: number) => void;
}

export function AnswerStream({
  phase,
  thinking,
  answer,
  citations,
  error,
  onRetry,
  onOpenFrame,
}: AnswerStreamProps) {
  const streaming = phase === "streaming";

  // The thinking trace is a *controlled* <details>: expanded while a new answer
  // streams in, then left as the user last set it. Tying `open` straight to
  // `streaming` would yank the panel shut the instant streaming finished — exactly
  // when the user may want to read it. Hooks run unconditionally, before any early
  // return (Rules-of-Hooks gate).
  const [thinkingOpen, setThinkingOpen] = useState(true);
  const wasStreaming = useRef(false);
  useEffect(() => {
    if (streaming && !wasStreaming.current) setThinkingOpen(true);
    wasStreaming.current = streaming;
  }, [streaming]);

  // Auto-follow the reasoning trace: while it streams, keep the latest line in view so it
  // doesn't scroll out of reach — but only when the user is already near the bottom, so
  // scrolling up to re-read isn't yanked back down. The trace now grows inline (no inner
  // scroll, #59), so follow the nearest scrollable ancestor (the shell <main> in the route,
  // the overlay's own pane in the Flow overlay) rather than the <pre> itself.
  const thinkingRef = useRef<HTMLPreElement>(null);
  useEffect(() => {
    const el = thinkingRef.current;
    if (!el || !streaming || !thinkingOpen) return;
    const scroller = nearestScrollable(el);
    if (!scroller) return;
    const nearBottom =
      scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 48;
    if (nearBottom) scroller.scrollTop = scroller.scrollHeight;
  }, [thinking, streaming, thinkingOpen]);

  // When the answer finishes streaming, move focus to the answer region so keyboard
  // users land on the result instead of being stranded at the query input, and screen
  // readers announce it (the prose isn't aria-live — that would read every streamed
  // token). It's a non-interactive region focused programmatically, so the visible ring
  // is suppressed; the answer prose is itself the obvious result.
  const answerRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (phase === "done") answerRef.current?.focus();
  }, [phase]);

  if (phase === "idle") return null;

  if (phase === "error") {
    return (
      <ErrorState
        title="Couldn't answer that"
        message={
          error ??
          "The answer model is unavailable. Make sure the inference sidecar is loaded, then try again."
        }
        onRetry={onRetry}
      />
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {thinking && (
        <details
          open={thinkingOpen}
          onToggle={(e) =>
            setThinkingOpen((e.currentTarget as HTMLDetailsElement).open)
          }
          className="rounded-panel border border-line bg-base"
        >
          <summary className="cursor-pointer select-none px-3 py-2 text-caption text-ink-muted font-body">
            Thinking
          </summary>
          <pre
            ref={thinkingRef}
            className="whitespace-pre-wrap px-3 pb-3 text-caption text-ink-faint font-mono"
          >
            {thinking}
          </pre>
        </details>
      )}

      <div
        ref={answerRef}
        tabIndex={-1}
        role="region"
        aria-label="Answer"
        className="prose prose-deck max-w-none focus-visible:outline-none"
      >
        <Markdown
          remarkPlugins={[remarkGfm]}
          components={{
            // Answer text is model output: open any link in the OS browser, never
            // navigate the app's own WebView (which would unmount the whole UI). Tauri v2
            // ignores plain `target="_blank"`, so http(s) links are routed through the
            // opener plugin (0.3.1 D4); browser-dev and non-http(s) links fall back to the
            // native `target="_blank"`. Used in the main Recall route and the Flow overlay.
            a: ({ href, children }) => (
              <a
                href={href}
                target="_blank"
                rel="noopener noreferrer"
                onClick={(e) => handleExternalLinkClick(e, href)}
              >
                {children}
              </a>
            ),
          }}
        >
          {answer}
        </Markdown>
        {streaming && (
          <span
            className="ml-0.5 inline-block h-4 w-2 animate-pulse bg-accent align-text-bottom"
            aria-label="Answering…"
          />
        )}
      </div>

      {citations.length > 0 && (
        <div className="flex flex-col gap-2">
          <span className="eyebrow">Frames checked</span>
          {/* Wrap onto rows instead of a nested horizontal scroller (D9 — one scroll
              context; Windows draws a permanent scrollbar for overflow-x-auto). */}
          <div className="flex flex-wrap gap-2">
            {citations.map((id) => (
              <CitationTile key={id} frameId={id} onOpenFrame={onOpenFrame} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
