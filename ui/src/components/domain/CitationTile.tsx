// CitationTile — a grounding citation rendered as a thumbnail tile that links to the
// source moment (UI_REFERENCE §4 Recall). Resolves the frame's thumbnail/time from the
// same `useFrame` cache the Moment screen uses. Shared by AnswerStream (Ask) and
// ReportView (Reports) so both render source-frame chips identically.
import { Link } from "react-router-dom";

import { useFrame } from "../../lib/ipc/queries";
import { FrameImage } from "./FrameImage";
import { absoluteTime, clockTime } from "../../lib/time";

export interface CitationTileProps {
  frameId: number;
}

export function CitationTile({ frameId }: CitationTileProps) {
  const { data } = useFrame(frameId);
  return (
    <Link
      to={`/timeline/${frameId}`}
      title={data ? absoluteTime(data.captured_at) : `Frame #${frameId}`}
      className="flex w-32 shrink-0 flex-col gap-1 rounded-chip border border-line bg-surface p-1 transition-colors duration-fast ease-ui hover:border-accent"
    >
      <FrameImage
        imagePath={data?.image_path ?? null}
        alt=""
        className="aspect-video w-full rounded-chip object-cover bg-overlay"
      />
      <span className="px-1 font-mono text-data text-ink-muted">
        {data ? clockTime(data.captured_at) : `#${frameId}`}
      </span>
    </Link>
  );
}
