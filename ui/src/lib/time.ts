// Human-relative timestamps with an absolute form for hover/title (UI_REFERENCE
// §9: "Timestamps human-relative with absolute on hover"). Every input is unix
// epoch milliseconds — the `captured_at` wire format. These run only inside the
// WebView, so the platform `Date` / `Intl` APIs are always available.

const SECOND = 1_000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

// Built once at module load (constructing an Intl formatter per call is costly).
// 24-hour clock for the instrument-readout feel; locale otherwise honored.
const dateTimeFmt = new Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "short",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});
const dateFmt = new Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "short",
  day: "numeric",
});
const clockFmt = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});

/**
 * A short, human-relative label: "just now" · "5m ago" · "3h ago" · "yesterday" ·
 * "4d ago", falling back to an absolute date for anything older than a week. A
 * future timestamp (clock skew) reads "just now" rather than a negative span.
 */
export function relativeTime(ms: number, now: number = Date.now()): string {
  const diff = now - ms;
  if (diff < 45 * SECOND) return "just now";
  if (diff < 90 * SECOND) return "1m ago";
  if (diff < HOUR) return `${Math.round(diff / MINUTE)}m ago`;
  if (diff < 2 * HOUR) return "1h ago";
  if (diff < DAY) return `${Math.round(diff / HOUR)}h ago`;
  if (diff < 2 * DAY) return "yesterday";
  if (diff < 7 * DAY) return `${Math.round(diff / DAY)}d ago`;
  return absoluteDate(ms);
}

/** Full date + time, e.g. "Jun 22, 2026, 14:32" — the hover/title form. */
export function absoluteTime(ms: number): string {
  return dateTimeFmt.format(new Date(ms));
}

/** Date only, e.g. "Jun 22, 2026". */
export function absoluteDate(ms: number): string {
  return dateFmt.format(new Date(ms));
}

/** Clock time only, e.g. "14:32" — compact timeline / tile labels (mono). */
export function clockTime(ms: number): string {
  return clockFmt.format(new Date(ms));
}

/**
 * Local-time filename stem for a saved recall report:
 * `screensearch-report-YYYY-MM-DD-HHmm` (0.3.1 D2, #65). Uses the running clock (a report
 * is saved "now", not at a captured instant) with zero-padded 24-hour local time. The
 * backend appends the `.md` extension and de-collides same-minute names (`-2`, `-3`, …).
 */
export function reportFileStem(ms: number = Date.now()): string {
  const d = new Date(ms);
  const y = d.getFullYear();
  const mo = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  const h = String(d.getHours()).padStart(2, "0");
  const mi = String(d.getMinutes()).padStart(2, "0");
  return `screensearch-report-${y}-${mo}-${day}-${h}${mi}`;
}
