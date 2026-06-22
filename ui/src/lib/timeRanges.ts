// Time-window helpers for the recall views. A `TimeRange` is half-open
// `[start, end)` in unix epoch milliseconds (see the binding). Day boundaries are
// snapped to **local** midnight so the windows are stable within a day — the same
// values across renders, so they don't thrash TanStack Query's structural keys.
import type { TimeRange } from "../bindings/TimeRange";

const DAY_MS = 86_400_000;

/** Local midnight (ms) of the day containing `at` (defaults to now). */
export function startOfLocalDay(at: number = Date.now()): number {
  const d = new Date(at);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** `[today 00:00, tomorrow 00:00)` in local time. */
export function todayRange(): TimeRange {
  const start = startOfLocalDay();
  return { start, end: start + DAY_MS };
}

/**
 * The last `days` whole local days, ending at tomorrow 00:00 so today is fully
 * included. `lastDaysRange(1)` is today; `lastDaysRange(7)` the last week.
 */
export function lastDaysRange(days: number): TimeRange {
  const end = startOfLocalDay() + DAY_MS;
  return { start: end - Math.max(1, days) * DAY_MS, end };
}
