// SearchResult — one hybrid-search hit as a list row: a small thumbnail, the
// matched OCR snippet, and capture context (UI_REFERENCE §5). Opens the moment at
// /timeline/:id. The FTS arm wraps matched terms in `[...]` (see store::search's
// `snippet(...)`); we render those segments in the accent so the match is visible
// without a second highlight pass. Vector-only hits carry a plain snippet (no
// brackets) and render as-is.
import { Link } from "react-router-dom";

import type { SearchHit } from "../../bindings/SearchHit";
import { FrameImage } from "./FrameImage";
import { absoluteTime, relativeTime } from "../../lib/time";

/** Splits an FTS snippet on its `[match]` delimiters, accenting the matched runs. */
function HighlightedSnippet({ text }: { text: string }) {
  const parts = text.split(/(\[[^\]]*\])/g);
  return (
    <>
      {parts.map((part, i) =>
        part.startsWith("[") && part.endsWith("]") ? (
          <mark key={i} className="bg-transparent text-accent">
            {part.slice(1, -1)}
          </mark>
        ) : (
          <span key={i}>{part}</span>
        ),
      )}
    </>
  );
}

export interface SearchResultProps {
  hit: SearchHit;
}

export function SearchResult({ hit }: SearchResultProps) {
  return (
    <Link
      to={`/timeline/${hit.frame_id}`}
      title={absoluteTime(hit.captured_at)}
      className="flex gap-3 rounded-panel border border-line bg-surface p-3 transition-colors duration-fast ease-ui hover:border-ink-faint"
    >
      <FrameImage
        imagePath={hit.image_path}
        alt=""
        className="h-16 w-28 shrink-0 rounded-chip object-cover bg-overlay"
      />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="line-clamp-2 text-body text-ink font-body">
          {hit.snippet ? <HighlightedSnippet text={hit.snippet} /> : <span className="text-ink-faint">No text recognized</span>}
        </p>
        <div className="flex items-center gap-2 text-caption text-ink-faint">
          <span className="font-mono text-data">{relativeTime(hit.captured_at)}</span>
          {hit.app_hint && (
            <>
              <span aria-hidden="true">·</span>
              <span className="truncate font-body">{hit.app_hint}</span>
            </>
          )}
        </div>
      </div>
    </Link>
  );
}
