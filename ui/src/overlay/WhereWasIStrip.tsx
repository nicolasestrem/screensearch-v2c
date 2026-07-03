// The overlay's empty-query state (0.3.0 PR6; UI_REFERENCE §4 Flow overlay row):
// the where-was-i strip. Purely presentational — the owning FlowOverlay holds the
// `useWhereWasI` query (so its Enter handler can open the same frame) and passes it in.
// Five states, tokens only: loading skeleton · error retry · null → honest "Nothing to
// resume yet" · populated → a "Jump back" row opening the Moment in the main window.
import { EmptyState, ErrorState, Skeleton } from "../components/primitives";
import { FrameImage } from "../components/domain";
import { IconMark } from "../components/icons";
import { openMoment } from "../lib/ipc/commands";
import type { useWhereWasI } from "../lib/ipc/queries";
import { clockTime } from "../lib/time";
import type { ResumeContext } from "../bindings/ResumeContext";

/** The one-line headline for a resume context: the window title (the "what") then the
 *  app (the "where"), or just the app when there's no title. */
function resumeHeadline(ctx: ResumeContext): string {
  return ctx.window_title ? `${ctx.window_title} — ${ctx.app}` : ctx.app;
}

interface WhereWasIStripProps {
  resume: ReturnType<typeof useWhereWasI>;
}

export function WhereWasIStrip({ resume }: WhereWasIStripProps) {
  if (resume.isLoading) {
    return (
      <div className="flex items-center gap-3 p-3">
        <Skeleton className="h-14 w-24 shrink-0" />
        <div className="flex flex-1 flex-col gap-2">
          <Skeleton className="h-5 w-2/3" />
          <Skeleton className="h-4 w-1/3" />
        </div>
      </div>
    );
  }
  if (resume.isError) {
    return (
      <ErrorState
        title="Couldn't check where you were"
        message={String(resume.error)}
        onRetry={() => resume.refetch()}
      />
    );
  }
  const ctx = resume.data;
  if (!ctx) {
    return (
      <EmptyState
        icon={<IconMark size={24} />}
        title="Nothing to resume yet"
        description="Type to search — ? to ask"
      />
    );
  }

  return (
    <div className="p-2">
      <button
        type="button"
        onClick={() => void openMoment(ctx.frame_id).catch(() => undefined)}
        className="flex w-full items-center gap-3 rounded-chip border border-line bg-surface p-2 text-left transition-colors duration-fast ease-ui hover:border-accent"
      >
        <FrameImage
          imagePath={ctx.image_path}
          imagePurged={ctx.image_purged}
          alt=""
          className="h-14 w-24 shrink-0 rounded-chip object-cover bg-overlay"
        />
        <span className="flex min-w-0 flex-1 flex-col gap-1">
          <span className="eyebrow text-ink-faint">Jump back</span>
          <span className="truncate text-body text-ink font-body">
            {resumeHeadline(ctx)}
          </span>
          <span className="font-mono text-data text-ink-faint">
            until {clockTime(ctx.span_end)}
          </span>
        </span>
      </button>
    </div>
  );
}
