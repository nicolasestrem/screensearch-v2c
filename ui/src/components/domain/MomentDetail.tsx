// MomentDetail — one captured frame in full (UI_REFERENCE §4 Moment / §5). The
// image (intrinsic w/h → no layout shift), its capture context, the deferred vision
// analysis (or the on-demand "Tag with vision" entry point when it hasn't run yet —
// the partial state), the tags, and the recognized text. Text is shown as
// content_text (the default-retrieval layer) with raw_text always available via a
// disclosure — raw is preserved and viewable even though search defaults to content
// (03 §3b). Purely presentational: the route owns data fetching and wires
// `onQueueVision` to the enqueue_vision mutation. Tokens only.
import type { FrameDetail } from "../../bindings/FrameDetail";
import { Button, Chip, Panel } from "../primitives";
import { FrameImage } from "./FrameImage";
import { IconSparkle, IconTag } from "../icons";
import { absoluteTime } from "../../lib/time";

export interface MomentDetailProps {
  detail: FrameDetail;
  /** Enqueue on-demand vision tagging for this frame. */
  onQueueVision: () => void;
  /** A vision job for this frame is in flight. */
  queueing: boolean;
}

/** A labelled context row; renders nothing when the value is absent. */
function Meta({ label, value }: { label: string; value: string | null | undefined }) {
  if (!value) return null;
  return (
    <div className="flex flex-col gap-1">
      <span className="eyebrow">{label}</span>
      <span className="break-words text-body text-ink font-body">{value}</span>
    </div>
  );
}

export function MomentDetail({ detail, onQueueVision, queueing }: MomentDetailProps) {
  const { vision } = detail;
  // A negative confidence is the "unknown" sentinel (the model gave no usable score);
  // surface it as n/a rather than a misleading -100%.
  const confidenceChip = vision ? (
    vision.confidence >= 0 ? (
      <Chip tone="accent">{Math.round(vision.confidence * 100)}%</Chip>
    ) : (
      <Chip tone="neutral">n/a</Chip>
    )
  ) : undefined;

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-[1.6fr_1fr]">
      {/* Image + recognized text. */}
      <div className="flex flex-col gap-4">
        <FrameImage
          imagePath={detail.image_path}
          intrinsicWidth={detail.width}
          intrinsicHeight={detail.height}
          alt={`Capture from ${absoluteTime(detail.captured_at)}`}
          className="h-auto w-full rounded-panel border border-line bg-overlay object-contain"
        />
        <Panel title="Recognized text">
          {detail.content_text ? (
            <pre className="max-h-80 overflow-auto whitespace-pre-wrap text-body text-ink-muted font-mono">
              {detail.content_text}
            </pre>
          ) : (
            <p className="text-body text-ink-faint font-body">No text was recognized in this frame.</p>
          )}
          {detail.raw_text && (
            <details className="mt-3 border-t border-line pt-3">
              <summary className="eyebrow cursor-pointer select-none text-ink-muted">
                Raw text{detail.raw_text !== detail.content_text ? " (includes app chrome)" : ""}
              </summary>
              <pre className="mt-2 max-h-80 overflow-auto whitespace-pre-wrap text-body text-ink-muted font-mono">
                {detail.raw_text}
              </pre>
            </details>
          )}
        </Panel>
      </div>

      {/* Context, vision, tags. */}
      <div className="flex flex-col gap-4">
        <Panel title="Context">
          <div className="flex flex-col gap-3">
            <Meta label="Captured" value={absoluteTime(detail.captured_at)} />
            <Meta label="App" value={detail.app_hint} />
            <Meta label="Window" value={detail.window_title} />
            <Meta label="URL" value={detail.browser_url} />
            <Meta label="Monitor" value={`#${detail.monitor_index} · ${detail.width}×${detail.height}`} />
          </div>
        </Panel>

        <Panel title="Vision" action={confidenceChip}>
          {vision ? (
            <div className="flex flex-col gap-3">
              <p className="text-body text-ink font-body">{vision.description}</p>
              <div className="flex flex-wrap items-center gap-2">
                {vision.activity_type && <Chip tone="neutral">{vision.activity_type}</Chip>}
                <Chip tone="neutral">{vision.model}</Chip>
              </div>
            </div>
          ) : (
            // Partial state: vision hasn't run — the on-demand entry point (`03 §5`).
            <div className="flex flex-col items-start gap-3">
              <p className="text-body text-ink-muted font-body">
                This frame hasn't been tagged by vision yet.
              </p>
              <Button
                variant="primary"
                leadingIcon={<IconSparkle size={16} />}
                onClick={onQueueVision}
                disabled={queueing}
              >
                {queueing ? "Queuing…" : "Tag with vision"}
              </Button>
            </div>
          )}
        </Panel>

        {detail.tags.length > 0 && (
          <Panel title="Tags">
            <div className="flex flex-wrap gap-2">
              {detail.tags.map((t) => (
                <Chip key={t} tone="neutral">
                  <IconTag size={12} />
                  {t}
                </Chip>
              ))}
            </div>
          </Panel>
        )}
      </div>
    </div>
  );
}
