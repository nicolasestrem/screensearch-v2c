// ReportView — renders a finished recall report (UI_REFERENCE §4 Recall/reports,
// `docs/0.2.0.md` PR6): the markdown body (react-markdown + GFM, prose-deck tokens),
// clickable source-frame chips, Copy + `.md` download, and an honest footer stating
// the model, pass count, and coverage (covered/total periods, summarized/sampled
// frames). A no-evidence report renders its honest message with no chips/footer noise.
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

import { CitationTile } from "./CitationTile";
import { Button } from "../primitives";
import { toast } from "../../state/toastStore";
import type { ReportResponse } from "../../bindings/ReportResponse";

export interface ReportViewProps {
  report: ReportResponse;
}

/** How many citation chips to render before collapsing the rest into a count. */
const CITATION_CAP = 24;

function downloadMarkdown(report: ReportResponse) {
  const slug = report.range_label.replace(/[^a-z0-9]+/gi, "-").replace(/^-+|-+$/g, "") || "range";
  const blob = new Blob([report.markdown], { type: "text/markdown;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `recall-report-${slug}.md`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

async function copyMarkdown(markdown: string) {
  try {
    await navigator.clipboard.writeText(markdown);
    toast.success("Report copied to clipboard");
  } catch {
    toast.error("Couldn't copy — clipboard unavailable");
  }
}

export function ReportView({ report }: ReportViewProps) {
  const cited = report.cited_frame_ids;
  const shown = cited.slice(0, CITATION_CAP);
  const overflow = cited.length - shown.length;
  const hasEvidence = report.passes > 0;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="eyebrow">Report · {report.range_label}</span>
        <div className="flex items-center gap-2">
          <Button variant="ghost" size="sm" onClick={() => copyMarkdown(report.markdown)}>
            Copy
          </Button>
          <Button variant="secondary" size="sm" onClick={() => downloadMarkdown(report)}>
            Download .md
          </Button>
        </div>
      </div>

      <div className="prose prose-deck max-w-none">
        <Markdown
          remarkPlugins={[remarkGfm]}
          components={{
            // Report text is model output: open links in the OS browser, never the
            // app's own WebView (which would unmount the UI).
            a: ({ href, children }) => (
              <a href={href} target="_blank" rel="noopener noreferrer">
                {children}
              </a>
            ),
          }}
        >
          {report.markdown}
        </Markdown>
      </div>

      {cited.length > 0 && (
        <div className="flex flex-col gap-2">
          <span className="eyebrow">Source frames</span>
          <div className="flex gap-2 overflow-x-auto pb-1">
            {shown.map((id) => (
              <CitationTile key={id} frameId={id} />
            ))}
            {overflow > 0 && (
              <span className="flex shrink-0 items-center px-2 text-caption text-ink-muted">
                +{overflow} more
              </span>
            )}
          </div>
        </div>
      )}

      {hasEvidence && (
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-line pt-2 text-caption text-ink-faint">
          {report.model && <span className="font-mono">{report.model}</span>}
          <span>{report.passes} pass{report.passes === 1 ? "" : "es"}</span>
          <span>
            {report.periods_covered}/{report.periods_total} periods
          </span>
          <span>
            {report.frames_summarized}/{report.frames_sampled} frames summarized
          </span>
          {report.truncated && (
            <span className="text-warn">range trimmed to fit — more was captured than summarized</span>
          )}
        </div>
      )}
    </div>
  );
}
