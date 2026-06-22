// InsightsBars (UI_REFERENCE §5) — a ranked horizontal bar list for the Insights
// breakdowns (top apps, activity types). Lightweight token-styled divs (no chart
// library — protects the bundle budget, §8); bar widths are data-driven inline
// styles (the one thing a utility class can't express), colors stay on-token. Bars
// are proportional to the largest count in the set, so the list reads as a ranking.
export interface RankedItem {
  label: string;
  count: number;
}

export interface InsightsBarsProps {
  items: RankedItem[];
  /** Shown in place of the list when there is nothing to rank. */
  emptyLabel?: string;
}

export function InsightsBars({ items, emptyLabel = "No data yet." }: InsightsBarsProps) {
  if (items.length === 0) {
    return <p className="text-body text-ink-muted font-body">{emptyLabel}</p>;
  }
  const max = Math.max(...items.map((i) => i.count), 1);
  return (
    <ul className="flex flex-col gap-2">
      {items.map((item, i) => {
        // Floor the visible width so even the smallest count shows a sliver of bar.
        const pct = Math.max(2, Math.round((item.count / max) * 100));
        return (
          <li key={`${item.label}-${i}`} className="flex items-center gap-3">
            <span
              className="w-36 shrink-0 truncate text-body text-ink font-body"
              title={item.label}
            >
              {item.label}
            </span>
            <div className="relative h-6 min-w-0 flex-1 overflow-hidden rounded-chip bg-overlay">
              <div className="h-full rounded-chip bg-accent" style={{ width: `${pct}%` }} />
            </div>
            <span className="w-12 shrink-0 text-right font-mono text-data text-ink-muted">
              {item.count}
            </span>
          </li>
        );
      })}
    </ul>
  );
}
