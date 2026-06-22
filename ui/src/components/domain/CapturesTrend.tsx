// CapturesTrend (UI_REFERENCE §5) — capture density over the window for Insights.
// The store returns sparse, fixed-width buckets ascending by time (`insights.rs`
// reuses `timeline_buckets`); each bar is positioned at its true time offset within
// the range and gaps stay empty, so the chart reads as honest "when did I capture"
// rather than evenly spacing unequal periods. Token-styled divs, no chart lib (§8).
import type { TimeRange } from "../../bindings/TimeRange";
import type { TimelineBucket } from "../../bindings/TimelineBucket";
import { absoluteDate } from "../../lib/time";

export interface CapturesTrendProps {
  buckets: TimelineBucket[];
  range: TimeRange;
}

export function CapturesTrend({ buckets, range }: CapturesTrendProps) {
  const span = Math.max(1, range.end - range.start);
  const max = Math.max(...buckets.map((b) => b.count), 1);
  const total = buckets.reduce((sum, b) => sum + b.count, 0);

  return (
    <div className="flex flex-col gap-2">
      <div
        role="img"
        aria-label={`Captures over time: ${total} frames across ${buckets.length} active periods`}
        className="relative h-32 w-full overflow-hidden rounded-chip bg-overlay"
      >
        {buckets.map((b, i) => {
          const left = ((b.start - range.start) / span) * 100;
          const width = (Math.max(1, b.end - b.start) / span) * 100;
          const height = Math.max(4, (b.count / max) * 100);
          return (
            <div
              key={i}
              className="absolute bottom-0 bg-accent"
              style={{ left: `${left}%`, width: `${width}%`, height: `${height}%` }}
            />
          );
        })}
      </div>
      <div className="flex justify-between text-caption text-ink-faint font-mono">
        <span>{absoluteDate(range.start)}</span>
        <span>{absoluteDate(range.end - 1)}</span>
      </div>
    </div>
  );
}
