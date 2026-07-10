// SessionBands — an additive, kind-coded layer over the Scanline ribbon. The
// measured pixel geometry (including the tokenized 32px hit target) drives both
// deterministic collision packing and rendering, so CSS cannot create overlaps
// the packer did not see. Rows stay in normal flow and expand the ribbon as needed.
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";

import type { Session } from "../../bindings/Session";
import type { SessionKind } from "../../bindings/SessionKind";
import type { TimeRange } from "../../bindings/TimeRange";
import { absoluteTime } from "../../lib/time";
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

interface LayoutMetrics {
  width: number;
  hitTarget: number;
  gap: number;
}

interface PackedBand {
  session: Session;
  lane: number;
  leftPx: number;
  widthPx: number;
}

const EMPTY_METRICS: LayoutMetrics = { width: 0, hitTarget: 0, gap: 0 };

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

function cssTokenPx(style: CSSStyleDeclaration, token: string): number {
  const value = Number.parseFloat(style.getPropertyValue(token));
  return Number.isFinite(value) && value > 0 ? value : 0;
}

function packBands(
  sessions: Session[],
  rangeStart: number,
  rangeEnd: number,
  metrics: LayoutMetrics,
) {
  const { width, hitTarget, gap } = metrics;
  if (width <= 0 || hitTarget <= 0) {
    return { bands: [] as PackedBand[], laneCount: 1 };
  }

  const span = Math.max(1, rangeEnd - rangeStart);
  const minBandWidth = Math.min(hitTarget, width);
  const ordered = sessions
    .map((session) => {
      const start = Math.max(rangeStart, session.started_at);
      const end = Math.min(rangeEnd, session.ended_at ?? rangeEnd);
      const rawLeft = ((start - rangeStart) / span) * width;
      const rawRight = ((end - rangeStart) / span) * width;
      const renderedWidth = Math.min(
        width,
        Math.max(minBandWidth, rawRight - rawLeft),
      );
      const leftPx = Math.min(
        Math.max(0, rawLeft),
        Math.max(0, width - renderedWidth),
      );
      return {
        session,
        start,
        end,
        leftPx,
        widthPx: renderedWidth,
        rightPx: leftPx + renderedWidth,
      };
    })
    .filter((item) => item.end > item.start)
    .sort(
      (a, b) =>
        a.leftPx - b.leftPx ||
        a.rightPx - b.rightPx ||
        a.start - b.start ||
        a.session.id - b.session.id,
    );
  const laneEnds: number[] = [];
  const bands: PackedBand[] = [];

  for (const item of ordered) {
    let lane = laneEnds.findIndex((endPx) => item.leftPx >= endPx + gap);
    if (lane < 0) lane = laneEnds.length;
    laneEnds[lane] = item.rightPx;
    bands.push({
      session: item.session,
      lane,
      leftPx: item.leftPx,
      widthPx: item.widthPx,
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
  const measureRef = useRef<HTMLDivElement>(null);
  const [metrics, setMetrics] = useState<LayoutMetrics>(EMPTY_METRICS);

  useEffect(() => {
    const element = measureRef.current;
    if (!element) return;
    const measure = () => {
      const style = getComputedStyle(element);
      const next = {
        width: element.clientWidth,
        hitTarget: cssTokenPx(style, "--hit-min"),
        gap: cssTokenPx(style, "--space-1"),
      };
      setMetrics((current) =>
        current.width === next.width &&
        current.hitTarget === next.hitTarget &&
        current.gap === next.gap
          ? current
          : next,
      );
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const packed = useMemo(
    () => packBands(sessions, range.start, range.end, metrics),
    [sessions, range.start, range.end, metrics],
  );

  return (
    <div className="pointer-events-none relative z-rail min-h-32 pb-2 pt-6">
      <div ref={measureRef} className="relative mx-2">
        {loading ? (
          <div className="grid grid-rows-2 gap-1">
            <Skeleton className="h-hit-min w-1/3 border border-line bg-overlay" />
            <Skeleton className="h-hit-min w-1/4 border border-line bg-overlay" />
          </div>
        ) : error ? (
          <div className="flex min-h-hit-min items-center justify-center">
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
        ) : packed.bands.length > 0 ? (
          <div
            className="grid gap-1"
            style={{
              gridTemplateRows: `repeat(${packed.laneCount}, var(--hit-min))`,
            }}
            role="group"
            aria-label="Sessions in this timeline range"
          >
            {packed.bands.map(({ session, lane, leftPx, widthPx }) => {
              const kind = KIND_LABEL[session.kind];
              const endLabel = session.ended_at
                ? absoluteTime(session.ended_at)
                : "still running";
              const title = session.title?.trim() || "Untitled session";
              const context = [session.tool, session.host]
                .filter((value) => value != null)
                .join(", ");
              const label = `${kind} session: ${title}. ${absoluteTime(session.started_at)} to ${endLabel}${context ? `. ${context}` : ""}`;
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
                    "pointer-events-auto h-hit-min truncate rounded-chip border px-2 text-left font-display text-caption font-semibold uppercase tracking-eyebrow",
                    "transition-colors duration-fast ease-ui hover:bg-overlay",
                    KIND_STYLE[session.kind],
                  )}
                  style={{
                    gridRow: lane + 1,
                    gridColumn: 1,
                    marginLeft: `${leftPx}px`,
                    width: `${widthPx}px`,
                  }}
                >
                  {kind} · {absoluteTime(session.started_at)}–{endLabel} · {title}
                </button>
              );
            })}
          </div>
        ) : null}
      </div>
    </div>
  );
}
