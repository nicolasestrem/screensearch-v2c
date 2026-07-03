import type { SearchHit } from "../bindings/SearchHit";
import { FrameImage } from "../components/domain/FrameImage";
import { HighlightedSnippet } from "../components/domain/HighlightedSnippet";
import { cn } from "../lib/cn";
import { absoluteTime, relativeTime } from "../lib/time";

export interface OverlayResultRowProps {
  hit: SearchHit;
  id: string;
  selected: boolean;
  onOpen: () => void;
  onHover: () => void;
}

export function OverlayResultRow({ hit, id, selected, onOpen, onHover }: OverlayResultRowProps) {
  return (
    <button
      id={id}
      type="button"
      role="option"
      aria-selected={selected}
      title={absoluteTime(hit.captured_at)}
      onClick={onOpen}
      onMouseEnter={onHover}
      className={cn(
        "flex w-full items-center gap-3 rounded-chip border p-2 text-left",
        "transition-colors duration-fast ease-ui",
        selected ? "border-accent bg-accent-wash" : "border-line bg-surface hover:border-ink-faint",
      )}
    >
      <FrameImage
        imagePath={hit.image_path}
        imagePurged={hit.image_purged}
        alt=""
        className="h-14 w-24 shrink-0 rounded-chip object-cover bg-overlay"
      />
      <span className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="line-clamp-2 text-body text-ink font-body">
          {hit.snippet ? (
            <HighlightedSnippet text={hit.snippet} />
          ) : (
            <span className="text-ink-faint">No text recognized</span>
          )}
        </span>
        <span className="flex min-w-0 items-center gap-2 text-caption text-ink-faint">
          <span className="font-mono text-data">{relativeTime(hit.captured_at)}</span>
          {hit.app_hint && (
            <>
              <span aria-hidden="true">.</span>
              <span className="truncate font-body">{hit.app_hint}</span>
            </>
          )}
        </span>
      </span>
    </button>
  );
}
