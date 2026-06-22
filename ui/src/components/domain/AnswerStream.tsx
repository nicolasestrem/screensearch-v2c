// AnswerStream — renders the streamed `ask` answer (UI_REFERENCE §4 Recall/ask).
// Folds the useAsk view-model into: a collapsible *thinking* trace (native
// <details>, so keyboard + a11y come free), the answer prose (markdown via
// react-markdown + GFM, themed with the `prose-deck` typography tokens), and the
// grounding citations as clickable tiles that open each source moment. react-markdown
// is imported only here, so it ships in the /recall route chunk (§8). The idle phase
// renders nothing — the Recall screen owns the empty "ask a question" invitation.
import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

import type { AskPhase } from "../../lib/ipc/useAsk";
import { useFrame } from "../../lib/ipc/queries";
import { FrameImage } from "./FrameImage";
import { ErrorState } from "../primitives";
import { absoluteTime, clockTime } from "../../lib/time";

export interface AnswerStreamProps {
  phase: AskPhase;
  thinking: string;
  answer: string;
  citations: number[];
  error: string | null;
  /** Re-run the last question (shown on error). */
  onRetry?: () => void;
}

/** A citation rendered as a thumbnail tile → the source moment. Resolves the
 *  frame's thumbnail/time from cache (the same query the Moment screen uses). */
function CitationTile({ frameId }: { frameId: number }) {
  const { data } = useFrame(frameId);
  return (
    <Link
      to={`/timeline/${frameId}`}
      title={data ? absoluteTime(data.captured_at) : `Frame #${frameId}`}
      className="flex w-32 shrink-0 flex-col gap-1 rounded-chip border border-line bg-surface p-1 transition-colors duration-fast ease-ui hover:border-accent"
    >
      <FrameImage
        imagePath={data?.image_path ?? null}
        alt=""
        className="aspect-video w-full rounded-chip object-cover bg-overlay"
      />
      <span className="px-1 font-mono text-data text-ink-muted">
        {data ? clockTime(data.captured_at) : `#${frameId}`}
      </span>
    </Link>
  );
}

export function AnswerStream({ phase, thinking, answer, citations, error, onRetry }: AnswerStreamProps) {
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

  if (phase === "idle") return null;

  if (phase === "error") {
    return (
      <ErrorState
        title="Couldn't answer that"
        message={error ?? "The answer model is unavailable. Make sure the inference sidecar is loaded, then try again."}
        onRetry={onRetry}
      />
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {thinking && (
        <details
          open={thinkingOpen}
          onToggle={(e) => setThinkingOpen((e.currentTarget as HTMLDetailsElement).open)}
          className="rounded-panel border border-line bg-base"
        >
          <summary className="cursor-pointer select-none px-3 py-2 text-caption text-ink-muted font-body">
            Thinking
          </summary>
          <pre className="overflow-x-auto whitespace-pre-wrap px-3 pb-3 text-caption text-ink-faint font-mono">
            {thinking}
          </pre>
        </details>
      )}

      <div className="prose prose-deck max-w-none">
        <Markdown
          remarkPlugins={[remarkGfm]}
          components={{
            // Answer text is model output: open any link in the OS browser, never
            // navigate the app's own WebView (which would unmount the whole UI).
            a: ({ href, children }) => (
              <a href={href} target="_blank" rel="noopener noreferrer">
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
          <span className="eyebrow">Cited frames</span>
          <div className="flex gap-2 overflow-x-auto pb-1">
            {citations.map((id) => (
              <CitationTile key={id} frameId={id} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
