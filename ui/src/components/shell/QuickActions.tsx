// QuickActions — the NavRail-footer quick menu (0.3.2 PR3, #57; UI_REFERENCE §3). Two
// lifecycle toggles that mirror the tray's quick actions and round-trip through the same
// commands: Load/Unload the answer model, and Start/Stop vision tagging. Quiet by design
// (functional tones, never the accent — telemetry-footer discipline, like the version
// link). Hidden entirely outside the packaged app (browser dev), like UpdateIndicator.
//
// State is read, not tracked: the answer-model label follows the live `sidecar_status`
// (loaded = the answer model is the resident sidecar model); the vision label follows the
// live job counts (active = any vision_tag job pending/running). Both caches are kept live
// by useLiveEvents, so the labels flip on their own after the tray or Settings act too.
import { isTauri } from "@tauri-apps/api/core";

import { cn } from "../../lib/cn";
import { useJobStats, useSidecarStatus } from "../../lib/ipc/queries";
import {
  useCancelVision,
  useEnqueueVision,
  useLoadModel,
  useUnloadModel,
} from "../../lib/ipc/mutations";
import { toast } from "../../state/toastStore";

// Matches the footer Command button (NavRail) — a quiet lifecycle control, not an accent.
const QUICK_ACTION = cn(
  "flex items-center justify-between w-full gap-2 px-3 min-h-hit-min rounded-chip",
  "text-caption text-ink-muted border border-line hover:text-ink hover:border-ink-faint",
  "transition-colors duration-fast ease-ui",
  "disabled:opacity-50 disabled:pointer-events-none",
);

export function QuickActions() {
  // Hooks first (Rules of Hooks), then the runtime gate.
  const sidecar = useSidecarStatus();
  const jobs = useJobStats();
  const loadModel = useLoadModel();
  const unloadModel = useUnloadModel();
  const enqueueVision = useEnqueueVision();
  const cancelVision = useCancelVision();
  if (!isTauri()) return null;

  // The answer model is "loaded" only when it is the resident sidecar model (one model at
  // a time) — a resident vision model correctly reads as "Load answer model".
  const answerLoaded =
    sidecar.data?.state === "ready" && sidecar.data?.lane === "answer";
  const modelBusy = loadModel.isPending || unloadModel.isPending;

  const visionActive =
    (jobs.data?.vision_pending ?? 0) + (jobs.data?.vision_running ?? 0) > 0;
  const visionBusy = enqueueVision.isPending || cancelVision.isPending;

  const toggleModel = () => {
    if (answerLoaded) {
      unloadModel.mutate(undefined, {
        onSuccess: () => toast.success("Answer model unloaded"),
        onError: (e) => toast.error(String(e)),
      });
    } else {
      loadModel.mutate("answer", {
        onSuccess: () => toast.success("Answer model loaded"),
        onError: (e) => toast.error(String(e)),
      });
    }
  };

  const toggleVision = () => {
    if (visionActive) {
      cancelVision.mutate(undefined, {
        onSuccess: (n) =>
          toast.info(
            n > 0
              ? `Stopped vision tagging — cleared ${n} queued`
              : "Stopped vision tagging",
          ),
        onError: (e) => toast.error(String(e)),
      });
    } else {
      // Start = tag the untagged backlog now (capped server-side); Stop cancels it.
      enqueueVision.mutate(
        { kind: "range", start: 0, end: Date.now() },
        {
          onSuccess: (n) =>
            n > 0
              ? toast.success(`Vision tagging started — ${n} frames queued`)
              : toast.info("No untagged frames to tag"),
          onError: (e) => toast.error(String(e)),
        },
      );
    }
  };

  return (
    <>
      <button
        type="button"
        onClick={toggleModel}
        disabled={modelBusy}
        className={QUICK_ACTION}
      >
        <span>{answerLoaded ? "Unload answer model" : "Load answer model"}</span>
      </button>
      <button
        type="button"
        onClick={toggleVision}
        disabled={visionBusy}
        className={QUICK_ACTION}
      >
        <span>
          {visionActive ? "Stop vision tagging" : "Start vision tagging"}
        </span>
      </button>
    </>
  );
}
