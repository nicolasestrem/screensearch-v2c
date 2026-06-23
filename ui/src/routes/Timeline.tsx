// Timeline (/timeline) — the signature Scanline Timeline browser (UI_REFERENCE
// §1/§4). A density ribbon over the selected window, scrubbed by pointer or
// keyboard; Enter opens the moment nearest the scan-head (resolved server-side to a
// real frame id, so it lands precisely — not on a sampled thumbnail). Range presets
// scope the window (and the Recall search). All five states: loading skeleton,
// empty ("No captures in this range"), error+retry, partial (thumbnails resolving),
// populated. The scrub area never goes blank — empty windows show an invitation.
import { useEffect, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

import { Button, Chip, EmptyState, ErrorState, Panel, Skeleton } from "../components/primitives";
import { ScanlineTimeline } from "../components/domain";
import * as cmd from "../lib/ipc/commands";
import { useFrames, useTimeline } from "../lib/ipc/queries";
import { useUiStore } from "../state/uiStore";
import { toast } from "../state/toastStore";
import { absoluteDate, absoluteTime } from "../lib/time";
import { lastDaysRange } from "../lib/timeRanges";
import { cn } from "../lib/cn";
import { useAdaptiveBucketCount } from "../lib/useAdaptiveBucketCount";

const DEFAULT_BUCKETS = 240;
const THUMB_LIMIT = 300; // hover-preview frames sampled across the window
const PRESETS = [
  { label: "Today", days: 1 },
  { label: "7 days", days: 7 },
  { label: "30 days", days: 30 },
] as const;

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

export function Component() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const setSelectedRange = useUiStore((s) => s.setSelectedRange);

  const [days, setDays] = useState(1);
  const range = lastDaysRange(days);

  // Scan-head position (unix ms). Initialised from a `?t=` deep link (e.g. the deck
  // minimap) or defaults to the most recent edge of the window.
  const [position, setPosition] = useState(() => {
    const t = Number(searchParams.get("t"));
    return Number.isFinite(t) && t > 0 ? t : range.end - 1;
  });

  const [timelineMeasureRef, bucketCount] = useAdaptiveBucketCount(DEFAULT_BUCKETS, 120, 2000, 4);
  const timeline = useTimeline(range, bucketCount);
  const thumbs = useFrames(range, THUMB_LIMIT);

  // Keep the head inside the window when the range changes.
  useEffect(() => {
    setPosition((p) => clamp(p, range.start, range.end - 1));
  }, [range.start, range.end]);

  const pickRange = (d: number) => {
    setDays(d);
    const r = lastDaysRange(d);
    setSelectedRange(r);
    setPosition(r.end - 1);
  };

  const openAt = async (pos: number) => {
    try {
      const frame = await cmd.getNearestFrame(Math.round(pos), range);
      if (frame) navigate(`/timeline/${frame.frame_id}`);
      else toast.info("No capture near that time");
    } catch (e) {
      toast.error(String(e));
    }
  };

  const buckets = timeline.data ?? [];
  const hasData = buckets.length > 0;

  const rangeControl = (
    <div className="flex gap-1" role="group" aria-label="Time range">
      {PRESETS.map((p) => (
        <button
          key={p.days}
          type="button"
          aria-pressed={days === p.days}
          onClick={() => pickRange(p.days)}
          className={cn(
            "inline-flex items-center rounded-chip px-3 min-h-hit-min font-display uppercase tracking-eyebrow text-caption font-semibold",
            "transition-colors duration-fast ease-ui",
            days === p.days ? "bg-accent-wash text-accent" : "text-ink-muted hover:text-ink hover:bg-overlay",
          )}
        >
          {p.label}
        </button>
      ))}
    </div>
  );

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-4 p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-col">
          <span className="eyebrow">Timeline</span>
          <span className="text-body text-ink-muted font-body">
            {absoluteDate(range.start)} — {absoluteDate(range.end - 1)}
          </span>
        </div>
        {rangeControl}
      </div>

      <div ref={timelineMeasureRef}>
        <Panel
          title="Scanline"
          flush
          action={hasData ? <Chip tone="accent">{absoluteTime(position)}</Chip> : undefined}
        >
          {timeline.isLoading ? (
            <Skeleton className="h-24 w-full rounded-none" />
          ) : timeline.isError ? (
            <ErrorState
              title="Couldn't load the timeline"
              message={String(timeline.error)}
              onRetry={() => timeline.refetch()}
            />
          ) : !hasData ? (
            <EmptyState
              title="No captures in this range"
              description="Nothing was recorded in this window. Widen the range, or start capture from the Deck."
              action={
                <Button variant="secondary" onClick={() => navigate("/")}>
                  Back to Deck
                </Button>
              }
            />
          ) : (
            <div className="flex flex-col gap-2">
              <ScanlineTimeline
                buckets={buckets}
                range={range}
                position={position}
                onScrub={setPosition}
                onOpen={openAt}
                thumbnails={thumbs.data ?? []}
              />
              <p className="px-1 text-caption text-ink-faint font-body">
                Drag or use ← → to scrub (Shift for bigger steps, Home/End to jump). Enter opens the moment.
                {thumbs.isLoading ? " · Loading thumbnails…" : ""}
              </p>
            </div>
          )}
        </Panel>
      </div>
    </div>
  );
}
