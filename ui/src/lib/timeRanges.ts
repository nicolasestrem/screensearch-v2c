// Time-window helpers for the recall views. A `TimeRange` is half-open
// `[start, end)` in unix epoch milliseconds (see the binding). Day boundaries are
// snapped to **local** midnight so the windows are stable within a day — the same
// values across renders, so they don't thrash TanStack Query's structural keys.
import type { TimeRange } from "../bindings/TimeRange";

/** Local midnight (ms) of the day containing `at` (defaults to now). */
export function startOfLocalDay(at: number = Date.now()): number {
  const d = new Date(at);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

function localDayOffset(at: number, offsetDays: number): number {
  const d = new Date(at);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + offsetDays).getTime();
}

/** `[today 00:00, tomorrow 00:00)` in local time. */
export function todayRange(): TimeRange {
  const now = Date.now();
  return { start: localDayOffset(now, 0), end: localDayOffset(now, 1) };
}

/**
 * The last `days` whole local days, ending at tomorrow 00:00 so today is fully
 * included. `lastDaysRange(1)` is today; `lastDaysRange(7)` the last week.
 */
export function lastDaysRange(days: number): TimeRange {
  const now = Date.now();
  const count = Math.max(1, Math.round(days));
  return { start: localDayOffset(now, 1 - count), end: localDayOffset(now, 1) };
}
