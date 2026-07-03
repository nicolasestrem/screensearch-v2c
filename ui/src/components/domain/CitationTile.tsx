// CitationTile — a grounding citation rendered as a thumbnail tile that opens the
// source moment (UI_REFERENCE §4 Recall). Resolves the frame's thumbnail/time from the
// same `useFrame` cache the Moment screen uses. Shared by AnswerStream (Ask) and
// ReportView (Reports) so both render source-frame chips identically.

import { useFrame } from "../../lib/ipc/queries";
import { FrameImage } from "./FrameImage";
import { absoluteTime, clockTime } from "../../lib/time";

export interface CitationTileProps {
  frameId: number;
  onOpenFrame?: (frameId: number) => void;
}

export function CitationTile({ frameId, onOpenFrame }: CitationTileProps) {
  const { data } = useFrame(frameId);
  const className =
    "flex w-32 shrink-0 flex-col gap-1 rounded-chip border border-line bg-surface p-1 text-left transition-colors duration-fast ease-ui hover:border-accent";
  const body = (
    <>
      <FrameImage
        imagePath={data?.image_path ?? null}
        imagePurged={data?.image_purged ?? false}
        alt=""
        className="aspect-video w-full rounded-chip object-cover bg-overlay"
      />
      <span className="px-1 font-mono text-data text-ink-muted">
        {data ? clockTime(data.captured_at) : `#${frameId}`}
      </span>
    </>
  );

  return (
    <button
      type="button"
      title={data ? absoluteTime(data.captured_at) : `Frame #${frameId}`}
      onClick={() => onOpenFrame?.(frameId)}
      className={className}
    >
      {body}
    </button>
  );
}
