// StatusRail (top) — live telemetry: capture · DB · queue · sidecar/model, plus an
// aggregate readiness chip (UI_REFERENCE §3). All values come from TanStack Query
// caches that the live event stream keeps fresh (useLiveEvents). Outside the Tauri
// shell the readiness query errors and the rail shows an honest "Kernel offline".
import { Chip, Skeleton, Tooltip } from "../primitives";
import { IconCapture, IconCpu, IconDatabase, IconQueue } from "../icons";
import { useJobStats, useReadiness, useSidecarStatus } from "../../lib/ipc/queries";
import { statusLabel, statusTone, worstStatus } from "../../lib/status";

export function StatusRail() {
  const readiness = useReadiness();
  const jobStats = useJobStats();
  const sidecar = useSidecarStatus();

  return (
    <header className="flex items-center justify-between gap-4 h-12 px-4 bg-surface border-b border-line">
      <div className="flex items-baseline gap-3">
        <span className="font-display uppercase tracking-eyebrow text-subtitle text-ink">
          ScreenSearch
        </span>
        <span className="eyebrow text-ink-faint">Command Deck</span>
      </div>

      <div className="flex items-center gap-2">
        {readiness.isLoading && (
          <>
            <Skeleton className="w-24 h-6" />
            <Skeleton className="w-20 h-6" />
            <Skeleton className="w-24 h-6" />
          </>
        )}

        {readiness.isError && (
          <Tooltip label={String(readiness.error)} side="bottom">
            <Chip tone="danger" dot>
              Kernel offline
            </Chip>
          </Tooltip>
        )}

        {readiness.data && (
          <>
            <Tooltip label={readiness.data.capture.detail ?? "Capture loop"} side="bottom">
              <Chip tone={statusTone(readiness.data.capture.status)}>
                <IconCapture size={14} />
                {statusLabel(readiness.data.capture.status)}
              </Chip>
            </Tooltip>

            <Tooltip label={readiness.data.db.detail ?? "Data store"} side="bottom">
              <Chip tone={statusTone(readiness.data.db.status)}>
                <IconDatabase size={14} />
                DB
              </Chip>
            </Tooltip>

            <Tooltip
              label={
                jobStats.data
                  ? `pending ${jobStats.data.pending} · running ${jobStats.data.running} · done ${jobStats.data.done} · failed ${jobStats.data.failed}`
                  : "Enrichment queue"
              }
              side="bottom"
            >
              <Chip tone={jobStats.data && jobStats.data.failed > 0 ? "warn" : "neutral"}>
                <IconQueue size={14} />
                {jobStats.data ? jobStats.data.pending + jobStats.data.running : "—"}
              </Chip>
            </Tooltip>

            <Tooltip
              label={
                sidecar.data?.model
                  ? `${readiness.data.sidecar.detail ?? "Inference sidecar"} · ${sidecar.data.model}`
                  : (readiness.data.sidecar.detail ?? "Inference sidecar")
              }
              side="bottom"
            >
              <Chip tone={statusTone(readiness.data.sidecar.status)}>
                <IconCpu size={14} />
                {statusLabel(readiness.data.sidecar.status)}
              </Chip>
            </Tooltip>

            <Chip tone={statusTone(worstStatus(readiness.data))} dot>
              {statusLabel(worstStatus(readiness.data))}
            </Chip>
          </>
        )}
      </div>
    </header>
  );
}
