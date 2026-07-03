import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";

import { Button, EmptyState, ErrorState, Skeleton } from "../components/primitives";
import { IconRecall } from "../components/icons";
import { hideOverlay, openMoment, overlayShownAck } from "../lib/ipc/commands";
import { listenTo } from "../lib/ipc/events";
import { useAsk } from "../lib/ipc/useAsk";
import { useSearch, useSettings } from "../lib/ipc/queries";
import { cn } from "../lib/cn";
import type { SearchQuery } from "../bindings/SearchQuery";
import type { SearchHit } from "../bindings/SearchHit";
import { OverlayResultRow } from "./OverlayResultRow";
import { OverlayAsk } from "./OverlayAsk";

type Mode = "search" | "ask";

const SEARCH_DEBOUNCE_MS = 250;
const DEFAULT_RESULT_LIMIT = 8;
const ASK_MAX_TOKENS = 2048;
const LISTBOX_ID = "flow-overlay-results";

export function FlowOverlay() {
  const inputRef = useRef<HTMLInputElement>(null);
  const [mode, setMode] = useState<Mode>("search");
  const [text, setText] = useState("");
  const [debounced, setDebounced] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const settings = useSettings();
  const ask = useAsk();
  const resetAsk = ask.reset;
  const resultLimit = settings.data?.overlay_max_results ?? DEFAULT_RESULT_LIMIT;
  const normalizedText = text.trim();
  const committedSearchText = debounced.trim();
  const searchWaitingForCommit =
    mode === "search" && normalizedText.length > 0 && normalizedText !== committedSearchText;
  const searchMatchesInput =
    mode === "search" && normalizedText.length > 0 && normalizedText === committedSearchText;

  useEffect(() => {
    const t = setTimeout(() => setDebounced(text), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [text]);

  const query: SearchQuery = useMemo(
    () => ({
      text: debounced,
      limit: resultLimit,
      time_range: null,
      include_chrome: false,
    }),
    [debounced, resultLimit],
  );
  const search = useSearch(query, mode === "search", true);
  const hits = searchMatchesInput ? (search.data ?? []) : [];
  const activeHit = searchMatchesInput ? hits[Math.min(activeIndex, Math.max(hits.length - 1, 0))] : undefined;
  const busy =
    (mode === "search" && normalizedText.length > 0 && (searchWaitingForCommit || search.isFetching)) ||
    ask.phase === "streaming";

  useEffect(() => {
    setActiveIndex(0);
  }, [text, mode]);

  useEffect(() => {
    if (activeIndex >= hits.length) {
      setActiveIndex(Math.max(hits.length - 1, 0));
    }
  }, [activeIndex, hits.length]);

  useEffect(() => {
    let active = true;
    const unlisteners: Array<() => void> = [];
    const track = (p: Promise<() => void>) =>
      p
        .then((u) => {
          if (active) unlisteners.push(u);
          else u();
        })
        .catch(() => {
          /* no Tauri runtime in browser dev */
        });

    track(
      listenTo("overlay_shown", () => {
        inputRef.current?.focus();
        inputRef.current?.select();
        requestAnimationFrame(() => {
          requestAnimationFrame(() => {
            void overlayShownAck().catch(() => undefined);
          });
        });
      }),
    );
    track(
      listenTo("overlay_hidden", () => {
        resetAsk();
      }),
    );

    return () => {
      active = false;
      unlisteners.forEach((u) => u());
    };
  }, [resetAsk]);

  const openFrame = (frameId: number) => {
    void openMoment(frameId).catch(() => undefined);
  };

  const submitAsk = () => {
    const question = text.trim();
    if (!question) return;
    setMode("ask");
    void ask.ask({
      query: question,
      thinking: settings.data?.answer_thinking ?? true,
      max_tokens: ASK_MAX_TOKENS,
      top_k: null,
    });
  };

  const toggleMode = () => {
    setMode((m) => {
      const next = m === "search" ? "ask" : "search";
      if (next === "search") ask.reset();
      return next;
    });
  };

  const handleTextChange = (value: string) => {
    if (value.startsWith("?")) {
      setMode("ask");
      setText(value.slice(1).trimStart());
      return;
    }
    setText(value);
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      e.preventDefault();
      void hideOverlay().catch(() => undefined);
      return;
    }
    if (e.key === "Tab") {
      e.preventDefault();
      toggleMode();
      return;
    }
    if (e.key === "?" && text.length === 0) {
      e.preventDefault();
      setMode("ask");
      return;
    }
    if (mode === "search" && e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, Math.max(hits.length - 1, 0)));
      return;
    }
    if (mode === "search" && e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      if (mode === "ask") {
        submitAsk();
      } else if (searchWaitingForCommit) {
        setDebounced(text);
      } else if (activeHit) {
        openFrame(activeHit.frame_id);
      }
    }
  };

  return (
    <div className="flex h-full items-start justify-center p-3">
      <section className="w-full overflow-hidden rounded-panel border border-line bg-overlay shadow-scan">
        <div
          className={cn("h-1 bg-accent shadow-scan", busy && "scanlines scanlines-drift")}
          aria-hidden="true"
        />
        <div className="scanlines flex flex-col gap-3 p-3">
          <div className="flex items-center gap-2">
            <input
              ref={inputRef}
              value={text}
              onChange={(e) => handleTextChange(e.currentTarget.value)}
              onKeyDown={handleKeyDown}
              role="combobox"
              aria-label={mode === "search" ? "Flow search" : "Flow ask"}
              aria-controls={LISTBOX_ID}
              aria-expanded={mode === "search"}
              aria-activedescendant={
                mode === "search" && activeHit ? optionId(activeHit.frame_id) : undefined
              }
              autoFocus
              placeholder={mode === "search" ? "Search screen history" : "Ask about captured screens"}
              className="min-w-0 flex-1 rounded-chip border border-line bg-base px-3 min-h-hit-min text-body text-ink placeholder:text-ink-faint font-body transition-colors duration-fast ease-ui focus:border-accent"
            />
            <Button
              variant={mode === "search" ? "primary" : "secondary"}
              size="sm"
              onClick={() => {
                setMode("search");
                ask.reset();
              }}
            >
              Search
            </Button>
            <Button variant={mode === "ask" ? "primary" : "secondary"} size="sm" onClick={() => setMode("ask")}>
              Ask
            </Button>
          </div>

          <div className="max-h-96 min-h-72 overflow-auto rounded-panel border border-line bg-base">
            {mode === "search" ? (
              <SearchOverlayBody
                text={text}
                hits={hits}
                isFetching={search.isFetching || searchWaitingForCommit}
                isError={search.isError}
                error={search.error}
                onRetry={() => search.refetch()}
                onOpen={openFrame}
                activeIndex={activeIndex}
                onActiveIndexChange={setActiveIndex}
              />
            ) : (
              <OverlayAsk
                question={text}
                ask={ask}
                onSubmit={submitAsk}
                onOpenFrame={openFrame}
              />
            )}
          </div>
        </div>
      </section>
    </div>
  );
}

interface SearchOverlayBodyProps {
  text: string;
  hits: SearchHit[];
  isFetching: boolean;
  isError: boolean;
  error: unknown;
  onRetry: () => void;
  onOpen: (frameId: number) => void;
  activeIndex: number;
  onActiveIndexChange: (index: number) => void;
}

function SearchOverlayBody({
  text,
  hits,
  isFetching,
  isError,
  error,
  onRetry,
  onOpen,
  activeIndex,
  onActiveIndexChange,
}: SearchOverlayBodyProps) {
  if (text.trim().length === 0) {
    return (
      <EmptyState
        icon={<IconRecall size={24} />}
        title="Type to search your screen history"
        description="? to ask"
      />
    );
  }
  if (isError) {
    return <ErrorState title="Search failed" message={String(error)} onRetry={onRetry} />;
  }
  if (isFetching && hits.length === 0) {
    return (
      <div className="flex flex-col gap-2 p-3">
        {Array.from({ length: 4 }, (_, i) => (
          <div key={i} className="flex items-center gap-3">
            <Skeleton className="h-14 w-24 shrink-0" />
            <div className="flex flex-1 flex-col gap-2">
              <Skeleton className="h-5 w-full" />
              <Skeleton className="h-4 w-2/3" />
            </div>
          </div>
        ))}
      </div>
    );
  }
  if (hits.length === 0) {
    return (
      <EmptyState
        icon={<IconRecall size={24} />}
        title="No matches"
        description="Try different words, or open Recall for filters."
      />
    );
  }

  return (
    <div id={LISTBOX_ID} role="listbox" aria-label="Flow search results" className="flex flex-col gap-2 p-2">
      {hits.map((hit, index) => (
        <OverlayResultRow
          key={hit.frame_id}
          id={optionId(hit.frame_id)}
          hit={hit}
          selected={index === activeIndex}
          onOpen={() => onOpen(hit.frame_id)}
          onHover={() => onActiveIndexChange(index)}
        />
      ))}
    </div>
  );
}

function optionId(frameId: number): string {
  return `flow-option-${frameId}`;
}
