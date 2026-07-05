// UpdateIndicator — the NavRail-footer updater surface (0.3.2 PR2; UI_REFERENCE §3/§4/§5,
// `03 §11b`). Two quiet concerns, one component:
//
//   1. A **presence indicator** — a small tone dot + version — rendered ONLY when an update
//      exists (available / downloading / ready). It is a link to Settings, where the update
//      installs. Never a count (the no-badge-counts rule, D4); `idle`/`error` render nothing
//      here (an error surfaces only in the Settings · App line, §4). No update → zero
//      presence: the element does not exist, it is not "shown as zero".
//   2. A **manual "Check for updates"** control (the quick-menu affordance, #57 / gap #99),
//      styled like the footer's Command button — a lifecycle control, not an availability
//      badge, so it is always available.
//
// Pull-based + quiet (D1): the check runs in the background; a found update downloads on its
// own and installs only from Settings on a user-initiated restart. No modal, no nag, no toast.
// Hidden entirely outside the packaged app (browser dev), like the version link.
import { NavLink } from "react-router-dom";
import { isTauri } from "@tauri-apps/api/core";

import { cn } from "../../lib/cn";
import { useUpdateStatus } from "../../lib/ipc/queries";
import { useCheckForUpdates } from "../../lib/ipc/mutations";
import { toast } from "../../state/toastStore";

export function UpdateIndicator() {
  // Hooks first (Rules of Hooks), then the runtime gate.
  const status = useUpdateStatus();
  const check = useCheckForUpdates();
  if (!isTauri()) return null;

  const state = status.data?.state ?? "idle";
  const version =
    status.data?.state === "available" ||
    status.data?.state === "downloading" ||
    status.data?.state === "ready"
      ? status.data.version
      : null;
  const checking = check.isPending || state === "checking";
  const hasUpdate = version != null;
  const ready = state === "ready";
  // Disable the manual check while a check or a background download is in flight.
  const busy = checking || state === "available" || state === "downloading";

  const runCheck = () => {
    check.mutate(undefined, {
      onError: (e) => toast.error(`Could not check for updates: ${String(e)}`),
    });
  };

  return (
    <>
      {/* Presence indicator — only while an update exists (§4). Quiet functional tone
          (--ok when ready, --ink-muted while still downloading), never the bold accent,
          never a count. Links to Settings, where the restart-to-update lives. */}
      {hasUpdate && (
        <NavLink
          to="/settings"
          aria-label={
            ready
              ? `Update v${version} ready — open Settings to restart`
              : `Update v${version} downloading — open Settings`
          }
          className={cn(
            "flex items-center gap-2 px-3 min-h-hit-min rounded-chip",
            "text-caption font-body transition-colors duration-fast ease-ui",
            ready
              ? "text-ok hover:text-ink"
              : "text-ink-muted hover:text-ink",
          )}
        >
          <span
            className={cn(
              "w-2 h-2 rounded-full bg-current shrink-0",
              !ready && "opacity-70",
            )}
            aria-hidden="true"
          />
          <span className="font-mono text-data">v{version}</span>
          <span>update</span>
        </NavLink>
      )}

      {/* Manual check — the quick-menu affordance, styled like the Command button. */}
      <button
        type="button"
        onClick={runCheck}
        disabled={busy}
        className={cn(
          "flex items-center justify-between w-full gap-2 px-3 min-h-hit-min rounded-chip",
          "text-caption text-ink-muted border border-line hover:text-ink hover:border-ink-faint",
          "transition-colors duration-fast ease-ui",
          "disabled:opacity-50 disabled:pointer-events-none",
        )}
      >
        <span>{checking ? "Checking…" : "Check for updates"}</span>
      </button>
    </>
  );
}
