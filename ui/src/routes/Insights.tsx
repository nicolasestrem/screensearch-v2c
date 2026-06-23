// Insights (/insights) — truthful activity analytics from get_insights (UI_REFERENCE
// §3/§4). Every number is a real DB aggregate: captures over time, the top foreground
// apps, and the vision activity-type breakdown — no fabricated charts. States (§4):
// loading → skeleton; error → compute failed + retry; empty → honest "not enough
// history yet"; partial → vision tagging still in progress (the activity breakdown
// only covers tagged frames, labelled as such); populated → the charts.
import { useState } from "react";
import { useNavigate } from "react-router-dom";

import { Button, Chip, EmptyState, ErrorState, Panel, Skeleton } from "../components/primitives";
import { CapturesTrend, InsightsBars, type RankedItem } from "../components/domain";
import { IconInsights } from "../components/icons";
import { useInsights } from "../lib/ipc/queries";
import { lastDaysRange } from "../lib/timeRanges";
import { absoluteDate } from "../lib/time";
import { cn } from "../lib/cn";
import { useAdaptiveBucketCount } from "../lib/useAdaptiveBucketCount";

const PRESETS = [
  { label: "Today", days: 1 },
  { label: "7 days", days: 7 },
  { label: "30 days", days: 30 },
] as const;

function InsightsSkeleton() {
  return (
    <div className="mx-auto flex max-w-4xl flex-col gap-4 p-6">
      <Skeleton className="h-12 w-full" />
      <Skeleton className="h-44 w-full" />
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Skeleton className="h-56 w-full" />
        <Skeleton className="h-56 w-full" />
      </div>
    </div>
  );
}

export function Component() {
  const navigate = useNavigate();
  const [days, setDays] = useState(7);
  const range = lastDaysRange(days);
  const [trendMeasureRef, bucketCount] = useAdaptiveBucketCount(48, 24, 720, 6);
  const insights = useInsights(range, bucketCount);

  const rangeControl = (
    <div className="flex gap-1" role="group" aria-label="Time range">
      {PRESETS.map((p) => (
        <button
          key={p.days}
          type="button"
          aria-pressed={days === p.days}
          onClick={() => setDays(p.days)}
          className={cn(
            "inline-flex items-center rounded-chip px-3 min-h-hit-min font-display uppercase tracking-eyebrow text-caption font-semibold",
            "transition-colors duration-fast ease-ui",
            days === p.days
              ? "bg-accent-wash text-accent"
              : "text-ink-muted hover:text-ink hover:bg-overlay",
          )}
        >
          {p.label}
        </button>
      ))}
    </div>
  );

  const header = (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div className="flex flex-col">
        <span className="eyebrow">Insights</span>
        <span className="text-body text-ink-muted font-body">
          {absoluteDate(range.start)} — {absoluteDate(range.end - 1)}
        </span>
      </div>
      {rangeControl}
    </div>
  );

  if (insights.isLoading) return <InsightsSkeleton />;

  if (insights.isError || !insights.data) {
    return (
      <div className="mx-auto flex max-w-4xl flex-col gap-4 p-6">
        {header}
        <ErrorState
          title="Couldn't compute insights"
          message={String(insights.error ?? "The aggregate query failed.")}
          onRetry={() => insights.refetch()}
        />
      </div>
    );
  }

  const data = insights.data;
  const { total_frames: total, tagged_frames: tagged } = data;

  // Empty: no captures landed in this window at all.
  if (total === 0) {
    return (
      <div className="mx-auto flex max-w-4xl flex-col gap-4 p-6">
        {header}
        <Panel title="Activity">
          <EmptyState
            icon={<IconInsights size={28} />}
            title="Not enough history yet"
            description="Nothing was captured in this window. Widen the range, or start capture from the Deck — insights are computed from your own history only."
            action={
              <Button variant="secondary" onClick={() => navigate("/")}>
                Go to Deck
              </Button>
            }
          />
        </Panel>
      </div>
    );
  }

  const topApps: RankedItem[] = data.top_apps.map((a) => ({
    label: a.app ?? "Unknown",
    count: a.count,
  }));
  const activities: RankedItem[] = data.activity_breakdown.map((a) => ({
    label: a.activity ?? "Unlabeled",
    count: a.count,
  }));

  const taggedPct = total > 0 ? Math.round((tagged / total) * 100) : 0;
  // Partial: some frames are still untagged, so the activity breakdown is a subset.
  const partial = tagged < total;

  return (
    <div className="mx-auto flex max-w-4xl flex-col gap-4 p-6">
      {header}

      <div className="flex flex-wrap gap-2">
        <Chip tone="accent">{total} captures</Chip>
        <Chip tone={tagged > 0 ? "ok" : "neutral"}>{tagged} tagged</Chip>
        {partial && <Chip tone="warn">{taggedPct}% tagged</Chip>}
      </div>

      <div ref={trendMeasureRef}>
        <Panel title="Captures over time">
          <CapturesTrend buckets={data.captures} range={range} />
        </Panel>
      </div>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Panel title="Top apps">
          <InsightsBars items={topApps} emptyLabel="No foreground app was recorded." />
        </Panel>

        <Panel
          title="Activities"
          action={partial ? <Chip tone="warn">tagged only</Chip> : undefined}
        >
          <div className="flex flex-col gap-3">
            {partial && (
              <p className="text-caption text-ink-faint font-body">
                Based on the {tagged} tagged {tagged === 1 ? "frame" : "frames"} so far. Tag more from
                a moment, or enable a schedule in Settings.
              </p>
            )}
            <InsightsBars
              items={activities}
              emptyLabel="No frames tagged yet — queue vision from a moment to see activities here."
            />
          </div>
        </Panel>
      </div>
    </div>
  );
}
