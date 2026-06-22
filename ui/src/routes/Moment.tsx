// Moment (/timeline/:id) — one captured frame in full (UI_REFERENCE §3/§4). Deep-
// linkable: the image, recognized text, vision tags, context, and the on-demand
// "Tag with vision" action. States: loading skeleton; error (load failed) → retry;
// "gone" (unknown / deleted id) → explain + back; partial (vision not yet tagged) →
// queue action (inside MomentDetail); populated. A context strip + prev/next walk
// the neighbouring captures.
import { useNavigate, useParams } from "react-router-dom";

import { Button, ErrorState, Panel, Skeleton } from "../components/primitives";
import { FrameTile, MomentDetail } from "../components/domain";
import { IconArrowLeft, IconChevronLeft, IconChevronRight } from "../components/icons";
import { useFrame, useFrameContext } from "../lib/ipc/queries";
import { useEnqueueVision } from "../lib/ipc/mutations";
import { toast } from "../state/toastStore";

const NEIGHBOUR_HALF_MS = 30 * 60 * 1000; // ±30 min context window
const NEIGHBOUR_EACH = 12; // closest captures to load on each side of the anchor

export function Component() {
  const { id } = useParams();
  const navigate = useNavigate();
  const frameId = id != null && /^\d+$/.test(id) ? Number(id) : null;

  const detail = useFrame(frameId);
  const enqueue = useEnqueueVision();

  // Captures bracketing this frame (enabled once we know its capture time). Anchored
  // to `at` — the closest on each side — so prev/next reach the true neighbours even
  // in a busy session (a plain newest-first window would only return the latest
  // frames and drop the anchor, breaking navigation).
  const at = detail.data?.captured_at ?? 0;
  const neighbours = useFrameContext(at, NEIGHBOUR_HALF_MS, NEIGHBOUR_EACH, detail.data != null);

  const queueVision = () => {
    if (frameId == null) return;
    enqueue.mutate(
      { kind: "frame", frame_id: frameId },
      {
        onSuccess: (n) => toast.success(n > 0 ? "Vision queued for this frame" : "Already tagged or queued"),
        onError: (e) => toast.error(String(e)),
      },
    );
  };

  const backToTimeline = (
    <Button variant="ghost" size="sm" leadingIcon={<IconArrowLeft size={16} />} onClick={() => navigate("/timeline")}>
      Timeline
    </Button>
  );

  // Invalid route param → treat as a missing moment, not a crash.
  if (frameId == null) {
    return (
      <div className="p-6">
        <ErrorState
          title="Unknown moment"
          message="That timeline address isn't valid."
          onRetry={() => navigate("/timeline")}
          retryLabel="Back to timeline"
        />
      </div>
    );
  }

  if (detail.isLoading) {
    return (
      <div className="mx-auto flex max-w-6xl flex-col gap-4 p-6">
        <Skeleton className="h-8 w-40" />
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-[1.6fr_1fr]">
          <Skeleton className="aspect-video w-full" />
          <Skeleton className="h-64 w-full" />
        </div>
      </div>
    );
  }

  if (detail.isError) {
    return (
      <div className="p-6">
        <ErrorState
          title="Couldn't load this moment"
          message={String(detail.error)}
          onRetry={() => detail.refetch()}
        />
      </div>
    );
  }

  // Unknown / deleted frame — `get_frame` returned null.
  if (!detail.data) {
    return (
      <div className="p-6">
        <ErrorState
          title="This moment is gone"
          message="The frame may have been deleted or never existed."
          onRetry={() => navigate("/timeline")}
          retryLabel="Back to timeline"
        />
      </div>
    );
  }

  // Prev / next within the loaded context window (ascending by capture time, anchor
  // excluded). `at` is this frame's capture time, so prev = the closest capture before
  // it and next = the closest after — derived by time, not by locating the anchor in
  // the list (it isn't there).
  const sorted = [...(neighbours.data ?? [])].sort((a, b) => a.captured_at - b.captured_at);
  const before = sorted.filter((f) => f.captured_at < at);
  const prev = before.length > 0 ? before[before.length - 1] : null;
  const next = sorted.find((f) => f.captured_at > at) ?? null;
  const context = sorted.filter((f) => f.frame_id !== frameId);

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-4 p-6">
      <div className="flex items-center justify-between gap-3">
        {backToTimeline}
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            leadingIcon={<IconChevronLeft size={16} />}
            disabled={!prev}
            onClick={() => prev && navigate(`/timeline/${prev.frame_id}`)}
          >
            Prev
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={!next}
            onClick={() => next && navigate(`/timeline/${next.frame_id}`)}
          >
            Next
            <IconChevronRight size={16} />
          </Button>
        </div>
      </div>

      <MomentDetail detail={detail.data} onQueueVision={queueVision} queueing={enqueue.isPending} />

      {context.length > 0 && (
        <Panel title="Around this moment">
          <div className="flex gap-3 overflow-x-auto pb-1">
            {context.map((f) => (
              <FrameTile key={f.frame_id} frame={f} className="w-40 shrink-0" />
            ))}
          </div>
        </Panel>
      )}
    </div>
  );
}
