// FrameReconstruction — a text + layout reconstruction of a captured frame, drawn from
// its stored OCR/UIA spans (normalized [0,1] geometry + classified role). This is the
// durable "proof" shown in the Moment view once a frame's screenshot has been retention-
// degraded (the pixels are gone, but each word's position and line height are enough to
// redraw what was on screen). Positions/sizes come from span data; color and font are
// tokens only. Container-query height units (`cqh`) scale each word to its real line
// height so the layout reads at any container size.
import type { TextSpan } from "../../bindings/TextSpan";
import { cn } from "../../lib/cn";

export interface FrameReconstructionProps {
  spans: TextSpan[];
  /** Native frame dimensions — set the canvas aspect ratio. */
  width: number;
  height: number;
  className?: string;
}

/** Role → emphasis: content reads at full strength, chrome/system/background recede. */
const ROLE_TONE: Record<TextSpan["role"], string> = {
  content: "text-ink",
  chrome: "text-ink-faint",
  system: "text-ink-faint",
  background: "text-ink-faint",
  unknown: "text-ink-muted",
};

export function FrameReconstruction({ spans, width, height, className }: FrameReconstructionProps) {
  const aspect = width > 0 && height > 0 ? width / height : 16 / 9;

  return (
    <div
      className={cn(
        "relative w-full overflow-hidden rounded-panel border border-line bg-surface",
        className,
      )}
      style={{ aspectRatio: String(aspect), containerType: "size" }}
      role="img"
      aria-label="Text reconstruction of the captured screen"
    >
      {spans.length === 0 ? (
        <div className="absolute inset-0 flex items-center justify-center p-4 text-center">
          <span className="text-body text-ink-faint font-body">
            No recognized text to reconstruct for this frame.
          </span>
        </div>
      ) : (
        spans.map((s, i) => (
          <span
            key={i}
            className={cn(
              "absolute overflow-hidden whitespace-nowrap font-mono leading-none",
              ROLE_TONE[s.role],
            )}
            // Geometry is data, not design tokens: place each word at its normalized
            // position and scale it to ~90% of its line height (`cqh` = % of canvas height).
            style={{
              left: `${s.x * 100}%`,
              top: `${s.y * 100}%`,
              maxWidth: `${(1 - s.x) * 100}%`,
              fontSize: `${Math.max(s.h * 90, 0.5)}cqh`,
            }}
          >
            {s.text}
          </span>
        ))
      )}
    </div>
  );
}
