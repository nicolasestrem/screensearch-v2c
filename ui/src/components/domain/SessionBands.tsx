// SessionBands — an additive, kind-coded layer over the Scanline ribbon. The
// interval-packing pass is deterministic and is the only non-trivial layout work
// memoized for this surface. Bands remain native buttons with ≥32px hit targets.
import { useMemo, type PointerEvent as ReactPointerEvent } from "react";

import type { Session } from "../../bindings/Session";
import type { SessionKind } from "../../bindings/SessionKind";
import type { TimeRange } from "../../bindings/TimeRange";
import { clockTime } from "../../lib/time";
import { cn } from "../../lib/cn";
import { Skeleton } from "../primitives";

export interface SessionBandsProps {
  sessions: Session[];
  range: TimeRange;
  loading?: boolean;
  error?: string | null;
  onRetry?: () => void;
  onOpen: (sessionId: number) => void;
}

interface PackedBand {
  session: Session;
  lane: number;
  leftPct: number;
  widthPct: number;
}

const KIND_LABEL: Record<SessionKind, string> = {
  focus: "Focus",
  ai: "AI",
  meeting: "Meeting",
  other: "Other",
};

const KIND_STYLE: Record<SessionKind, string> = {
  focus: "border-ink-faint bg-overlay text-ink",
  ai: "border-ok bg-surface text-ok",
  meeting: "border-warn bg-surface text-warn",
  other: "border-line bg-transparent text-ink-muted",
};

function packBands(sessions: Session[], rangeStart: number, rangeEnd: number) {
  const span = Math.max(1, rangeEnd - rangeStart);
  const ordered = sessions
    .map((session) => ({
      session,
      start: Math.max(rangeStart, session.started_at),
      end: Math.min(rangeEnd, session.ended_at ?? rangeEnd),
    }))
    .filter((item) => item.end > item.start)
    .sort(
      (a, b) =>
        a.start - b.start || a.end - b.end || a.session.id - b.session.id,
    );
  const laneEnds: number[] = [];
  const bands: PackedBand[] = [];

  for (const item of ordered) {
    let lane = laneEnds.findIndex((end) => item.start >= end);
    if (lane < 0) lane = laneEnds.length;
    laneEnds[lane] = item.end;
    bands.push({
      session: item.session,
      lane,
      leftPct: ((item.start - rangeStart) / span) * 100,
      widthPct: ((item.end - item.start) / span) * 100,
    });
  }

  return { bands, laneCount: Math.max(1, laneEnds.length) };
}

function stopSliderPointer(e: ReactPointerEvent<HTMLButtonElement>) {
  e.stopPropagation();
}

export function SessionBands({
  sessions,
  range,
  loading = false,
  error = null,
  onRetry,
  onOpen,
}: SessionBandsProps) {
  const packed = useMemo(
    () => packBands(sessions, range.start, range.end),
    [sessions, range.start, range.end],
  );

  if (loading) {
    return (
      <div className="pointer-events-none absolute inset-x-2 bottom-2 top-6 flex items-end gap-2">
        <Skeleton className="h-hit-min w-1/3 border border-line bg-overlay" />
        <Skeleton className="h-hit-min w-1/4 border border-line bg-overlay" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="pointer-events-none absolute inset-x-2 bottom-2 top-6 flex items-center justify-center">
        <button
          type="button"
          className="pointer-events-auto min-h-hit-min rounded-chip border border-line bg-overlay px-3 font-body text-caption text-ink-muted hover:text-ink"
          onPointerDown={stopSliderPointer}
          onClick={(e) => {
            e.stopPropagation();
            onRetry?.();
          }}
          title={error}
        >
          Sessions unavailable · Retry
        </button>
      </div>
    );
  }

  if (packed.bands.length === 0) return null;

  return (
    <div
      className="pointer-events-none absolute inset-x-2 bottom-2 top-6 grid gap-1"
      style={{
        gridTemplateRows: `repeat(${packed.laneCount}, minmax(0, 1fr))`,
      }}
      role="group"
      aria-label="Sessions in this timeline range"
    >
      {packed.bands.map(({ session, lane, leftPct, widthPct }) => {
        const endLabel = session.ended_at
          ? clockTime(session.ended_at)
          : "still running";
        const title =
          session.title?.trim() || `${KIND_LABEL[session.kind]} session`;
        const context = [session.tool, session.host]
          .filter((value) => value != null)
          .join(", ");
        const label = `${title}, ${clockTime(session.started_at)} to ${endLabel}${context ? `, ${context}` : ""}`;
        return (
          <button
            key={session.id}
            type="button"
            aria-label={label}
            title={label}
            onPointerDown={stopSliderPointer}
            onDoubleClick={(e) => e.stopPropagation()}
            onClick={(e) => {
              e.stopPropagation();
              onOpen(session.id);
            }}
            className={cn(
              "pointer-events-auto min-h-hit-min min-w-hit-min truncate rounded-chip border px-2 text-left font-display text-caption font-semibold uppercase tracking-eyebrow",
              "transition-colors duration-fast ease-ui hover:bg-overlay",
              KIND_STYLE[session.kind],
            )}
            style={{
              gridRow: lane + 1,
              gridColumn: 1,
              marginLeft: `${leftPct}%`,
              width: `${widthPct}%`,
            }}
          >
            {title}
          </button>
        );
      })}
    </div>
  );
}
