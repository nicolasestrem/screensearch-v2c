// buildReportFooter — the single source of truth for a recall report's footer block
// (0.3.1 D3, #65): app version · model · time span · filters · coverage counts. Rendered
// on screen by `ReportView` AND appended to the copied/saved markdown, so the exported
// file is self-describing. Kept out of the component file so it stays a plain, reusable
// pure function (and doesn't trip react-refresh's component-only-export rule).
import { absoluteDate } from "./time";
import type { ReportResponse } from "../bindings/ReportResponse";
import type { ReportRequest } from "../bindings/ReportRequest";

/**
 * One plain-text footer block for a finished report. Returns null for a no-evidence
 * report (`passes <= 0` — no model ran, so there is nothing honest to footer).
 */
export function buildReportFooter(
  report: ReportResponse,
  request: Omit<ReportRequest, "request_id"> | null,
  appVersion: string | null,
): string | null {
  if (report.passes <= 0) return null;
  const segments: string[] = [];
  segments.push(appVersion ? `ScreenSearch v${appVersion}` : "ScreenSearch");
  if (report.model) segments.push(report.model);
  if (request) {
    // `time_range.end` is exclusive (the local midnight after the last covered day), so
    // the last covered instant is `end - 1`; dating that avoids labeling the day after.
    const startDate = absoluteDate(request.time_range.start);
    const endDate = absoluteDate(request.time_range.end - 1);
    segments.push(
      startDate === endDate ? startDate : `${startDate} – ${endDate}`,
    );
    // Reports carry no other filters (include_chrome is always false); kind + the optional
    // Custom focus prompt are the whole story.
    segments.push(
      request.kind === "custom" && request.prompt
        ? `custom (focus: ${request.prompt})`
        : request.kind,
    );
  }
  segments.push(`${report.passes} pass${report.passes === 1 ? "" : "es"}`);
  segments.push(`${report.periods_covered}/${report.periods_total} periods`);
  segments.push(
    `${report.frames_summarized}/${report.frames_sampled} frames summarized`,
  );
  if (report.truncated) {
    segments.push("range trimmed to fit — more was captured than summarized");
  }
  return segments.join(" · ");
}
