// Recall (/recall) — hybrid search + grounded Ask + recall Reports (UI_REFERENCE §3/§4).
// One screen, three modes. Search lists hybrid hits (virtualized — no full-list DOM, §8);
// Ask streams a grounded answer with collapsible thinking, premade prompt cards, and
// citation tiles; Reports builds a daily/weekly/custom summary over content text with
// live progress, source-frame chips, copy + .md download. Every state is explicit:
// search → invite / loading / no-match / error / results; ask → invite(+cards) /
// streaming / done / error; reports → invite / generating / done / error. A banner
// flags degraded modes. Never a zero-result dead end.
import { useEffect, useMemo, useRef, useState, type FormEvent, type ReactNode } from "react";
import { useVirtualizer, type Virtualizer } from "@tanstack/react-virtual";

import { Button, Chip, EmptyState, ErrorState, Skeleton } from "../components/primitives";
import {
  SearchResult,
  AnswerStream,
  PromptCardGrid,
  ReportBuilder,
  ReportView,
} from "../components/domain";
import { IconRecall, IconSparkle, IconInsights } from "../components/icons";
import { useReadiness, useSearch, useSettings } from "../lib/ipc/queries";
import { useAsk } from "../lib/ipc/useAsk";
import { useReport } from "../lib/ipc/useReport";
import { useUiStore } from "../state/uiStore";
import { cn } from "../lib/cn";
import type { SearchQuery } from "../bindings/SearchQuery";
import type { SearchHit } from "../bindings/SearchHit";

type Mode = "search" | "ask" | "reports";

const SEARCH_LIMIT = 100;
const SEARCH_DEBOUNCE_MS = 250;
// Generation budget (n_predict) for an Ask reply. The default answer models are *reasoning*
// models that emit a `<think>…</think>` trace before the answer; 512 was exhausted mid-thought,
// so the answer was truncated to nothing (only the Thinking box showed). 2048 leaves room to
// finish reasoning *and* produce the answer while still preserving most of the 8K context
// window for retrieved snippets (`answer.rs::build_messages` reserves this from the budget).
const ASK_MAX_TOKENS = 2048;
const ROW_ESTIMATE = 104;

const MODES: { value: Mode; label: string; icon: ReactNode }[] = [
  { value: "search", label: "Search", icon: <IconRecall size={16} /> },
  { value: "ask", label: "Ask", icon: <IconSparkle size={16} /> },
  { value: "reports", label: "Reports", icon: <IconInsights size={16} /> },
];

export function Component() {
  const [mode, setMode] = useState<Mode>("search");
  const [text, setText] = useState("");
  const [debounced, setDebounced] = useState("");
  const selectedRange = useUiStore((s) => s.selectedRange);

  const readiness = useReadiness();
  const settings = useSettings();
  const ask = useAsk();
  const report = useReport();

  // Content-text (default) vs raw/app-chrome search (03 §3b). `null` follows the
  // user's configured default (`text.include_chrome_default`) until they toggle it.
  const [chromeOverride, setChromeOverride] = useState<boolean | null>(null);
  const includeChrome = chromeOverride ?? (settings.data?.text_include_chrome_default ?? false);

  // Debounce keystrokes into the committed search term (live search, bounded).
  useEffect(() => {
    const t = setTimeout(() => setDebounced(text), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [text]);

  const query: SearchQuery = useMemo(
    // Default retrieval is cleaned content text; `include_chrome` also searches raw /
    // app-chrome text so suppressed static terms are still findable (03 §3b).
    () => ({
      text: debounced,
      limit: SEARCH_LIMIT,
      time_range: selectedRange,
      include_chrome: includeChrome,
    }),
    [debounced, selectedRange, includeChrome],
  );
  const search = useSearch(query, mode === "search");

  const hits = mode === "search" ? (search.data ?? []) : [];
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: hits.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_ESTIMATE,
    overscan: 8,
  });

  // Fill + submit an Ask question (shared by the form and the premade cards). The
  // per-request `top_k` is left null → the configured `retrieval.default_top_k`.
  const askQuery = (q: string) => {
    const question = q.trim();
    if (!question) return;
    ask.ask({
      query: question,
      thinking: settings.data?.answer_thinking ?? true,
      max_tokens: ASK_MAX_TOKENS,
      top_k: null,
    });
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (mode === "ask") {
      askQuery(text);
    } else {
      setDebounced(text); // commit immediately on Enter
    }
  };

  const switchMode = (next: Mode) => {
    setMode(next);
    if (next === "ask") ask.reset();
  };

  const embedReady = readiness.data?.embed_model.status === "ready";
  const sidecarStatus = readiness.data?.sidecar.status;
  const sidecarDown = sidecarStatus === "unavailable" || sidecarStatus === "error";

  return (
    <div className="mx-auto flex h-full w-full max-w-4xl flex-col gap-4 p-6">
      {/* Mode toggle. */}
      <div className="flex gap-1" role="tablist" aria-label="Recall mode">
        {MODES.map((m) => (
          <button
            key={m.value}
            type="button"
            role="tab"
            aria-selected={mode === m.value}
            onClick={() => switchMode(m.value)}
            className={cn(
              "inline-flex items-center gap-2 rounded-chip px-3 min-h-hit-min font-display uppercase tracking-eyebrow text-caption font-semibold",
              "transition-colors duration-fast ease-ui",
              mode === m.value
                ? "bg-accent-wash text-accent"
                : "text-ink-muted hover:text-ink hover:bg-overlay",
            )}
          >
            {m.icon}
            {m.label}
          </button>
        ))}
      </div>

      {/* Query input (search + ask). Reports has its own range builder instead. */}
      {mode !== "reports" && (
        <form onSubmit={submit} className="flex gap-2">
          <input
            type="text"
            value={text}
            onChange={(e) => setText(e.target.value)}
            autoFocus
            placeholder={
              mode === "search"
                ? "Search your screen history…"
                : "Ask a question about what you've seen…"
            }
            aria-label={mode === "search" ? "Search query" : "Question"}
            className="min-w-0 flex-1 rounded-chip border border-line bg-base px-3 min-h-hit-min text-body text-ink placeholder:text-ink-faint font-body transition-colors duration-fast ease-ui focus:border-accent"
          />
          <Button type="submit" variant="primary" disabled={mode === "ask" && ask.phase === "streaming"}>
            {mode === "search" ? "Search" : ask.phase === "streaming" ? "Asking…" : "Ask"}
          </Button>
        </form>
      )}

      {/* Reports range builder. */}
      {mode === "reports" && (
        <ReportBuilder
          busy={report.phase === "generating"}
          onCancel={report.cancel}
          onGenerate={report.generate}
        />
      )}

      {/* Content vs raw/app-chrome retrieval (search mode only, 03 §3b). */}
      {mode === "search" && (
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            role="switch"
            aria-checked={includeChrome}
            onClick={() => setChromeOverride(!includeChrome)}
            className={cn(
              "inline-flex items-center gap-2 rounded-chip border px-3 min-h-hit-min text-caption font-display uppercase tracking-eyebrow",
              "transition-colors duration-fast ease-ui",
              includeChrome
                ? "border-accent text-accent bg-accent-wash"
                : "border-line text-ink-muted hover:text-ink",
            )}
          >
            {includeChrome ? "Including app chrome + raw text" : "Content text only"}
          </button>
          <span className="text-caption text-ink-faint">
            {includeChrome
              ? "Searching everything on screen, including toolbars and labels."
              : "Ignoring static toolbars, taskbars, and repeated labels."}
          </span>
        </div>
      )}

      {/* Degraded-mode banners (truthful, not blocking). */}
      {mode === "search" && readiness.data && !embedReady && (
        <Chip tone="warn">Searching text only — semantic search lights up once the embedding model loads</Chip>
      )}
      {mode !== "search" && sidecarDown && (
        <Chip tone="warn">
          Answer model not loaded{readiness.data?.sidecar.detail ? ` — ${readiness.data.sidecar.detail}` : ""}
        </Chip>
      )}

      {/* Body (single stable scroll container so the virtualizer ref is steady). */}
      <div ref={parentRef} className="min-h-0 flex-1 overflow-auto">
        {mode === "search" ? (
          <SearchBody
            query={query}
            isFetching={search.isFetching}
            isError={search.isError}
            error={search.error}
            onRetry={() => search.refetch()}
            hits={hits}
            virtualizer={virtualizer}
          />
        ) : mode === "ask" ? (
          ask.phase === "idle" ? (
            <div className="flex flex-col gap-6">
              <EmptyState
                icon={<IconSparkle size={28} />}
                title="Ask about what you've seen"
                description="Questions are answered from your captured screens, with the source frames cited. Try a card below, or ask your own."
              />
              <PromptCardGrid onPick={(p) => { setText(p); askQuery(p); }} />
            </div>
          ) : (
            <AnswerStream
              phase={ask.phase}
              thinking={ask.thinking}
              answer={ask.answer}
              citations={ask.citations}
              error={ask.error}
              onRetry={() => askQuery(text)}
            />
          )
        ) : (
          <ReportBody
            phase={report.phase}
            progress={report.progress}
            result={report.result}
            error={report.error}
          />
        )}
      </div>
    </div>
  );
}

interface ReportBodyProps {
  phase: ReturnType<typeof useReport>["phase"];
  progress: ReturnType<typeof useReport>["progress"];
  result: ReturnType<typeof useReport>["result"];
  error: string | null;
}

function ReportBody({ phase, progress, result, error }: ReportBodyProps) {
  if (phase === "idle") {
    return (
      <EmptyState
        icon={<IconInsights size={28} />}
        title="Build a recall report"
        description="Pick a range above and Generate. Reports summarize your captured content — not toolbars or app chrome — and cite the frames behind every point."
      />
    );
  }
  if (phase === "error") {
    return (
      <ErrorState
        title="Couldn't build that report"
        message={error ?? "The answer model is unavailable. Make sure the inference sidecar is loaded, then try again."}
      />
    );
  }
  if (phase === "generating") {
    return (
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-2 text-body text-ink-muted font-body">
          <span className="inline-block h-3 w-3 animate-pulse rounded-full bg-accent" aria-hidden />
          {progress ? progress.stage : "Starting…"}
          {progress && progress.total > 0 ? ` (${progress.done}/${progress.total})` : ""}
        </div>
        <span className="text-caption text-ink-faint">
          Weekly reports summarize each active day in turn, so they take a little longer.
        </span>
      </div>
    );
  }
  // done
  return result ? <ReportView report={result} /> : null;
}

interface SearchBodyProps {
  query: SearchQuery;
  isFetching: boolean;
  isError: boolean;
  error: unknown;
  onRetry: () => void;
  hits: SearchHit[];
  virtualizer: Virtualizer<HTMLDivElement, Element>;
}

function SearchBody({ query, isFetching, isError, error, onRetry, hits, virtualizer }: SearchBodyProps) {
  // Invite: nothing typed yet.
  if (query.text.trim().length === 0) {
    return (
      <EmptyState
        icon={<IconRecall size={28} />}
        title="Search your screen history"
        description="Find any moment by the text on screen or what it was about. Results link straight to the captured frame."
      />
    );
  }
  if (isError) {
    return <ErrorState title="Search failed" message={String(error)} onRetry={onRetry} />;
  }
  if (isFetching && hits.length === 0) {
    return (
      <div className="flex flex-col gap-3">
        {Array.from({ length: 5 }, (_, i) => (
          <Skeleton key={i} className="h-24 w-full" />
        ))}
      </div>
    );
  }
  if (hits.length === 0) {
    return (
      <EmptyState
        icon={<IconRecall size={28} />}
        title="No matches"
        description="Try different words, or widen the time range from the timeline."
      />
    );
  }

  return (
    <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
      {virtualizer.getVirtualItems().map((row) => {
        const hit = hits[row.index];
        return (
          <div
            key={hit.frame_id}
            data-index={row.index}
            ref={virtualizer.measureElement}
            className="absolute left-0 top-0 w-full pb-3"
            style={{ transform: `translateY(${row.start}px)` }}
          >
            <SearchResult hit={hit} />
          </div>
        );
      })}
    </div>
  );
}
