// FrameTile — a clickable thumbnail card for a FrameMeta (deck recents, moment
// neighbours). Opens the moment at /timeline/:id. Time is shown relative on the
// face with the absolute value on hover (UI_REFERENCE §9); the foreground app, when
// known, labels the capture. Tokens only; the whole tile is one ≥32px hit target.
import { Link } from "react-router-dom";

import type { FrameMeta } from "../../bindings/FrameMeta";
import { FrameImage } from "./FrameImage";
import { absoluteTime, clockTime } from "../../lib/time";
import { cn } from "../../lib/cn";

export interface FrameTileProps {
  frame: FrameMeta;
  className?: string;
}

export function FrameTile({ frame, className }: FrameTileProps) {
  return (
    <Link
      to={`/timeline/${frame.frame_id}`}
      title={absoluteTime(frame.captured_at)}
      className={cn(
        "group flex flex-col gap-2 rounded-panel border border-line bg-surface p-2",
        "transition-colors duration-fast ease-ui hover:border-ink-faint",
        className,
      )}
    >
      <FrameImage
        imagePath={frame.image_path}
        alt={`Capture at ${clockTime(frame.captured_at)}`}
        className="aspect-video w-full rounded-chip object-cover bg-overlay"
      />
      <div className="flex items-center justify-between gap-2">
        <span className="font-mono text-data text-ink-muted">{clockTime(frame.captured_at)}</span>
        {frame.app_hint && (
          <span className="truncate text-caption text-ink-faint font-body">{frame.app_hint}</span>
        )}
      </div>
    </Link>
  );
}
