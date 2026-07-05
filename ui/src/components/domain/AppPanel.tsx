// AppPanel — the Settings · App section (0.3.2 PR2; UI_REFERENCE §3/§4, `03 §11b`).
//
// PR2 scope: the auto-update surface (status line + manual "Check for updates" +
// "Restart to update") and the version + repo link restated where a user looks for it.
// The run-at-startup and close-to-tray settings (D1/D3) join this section in PR3, and PR5
// owns its final placement in the Essentials tier — this panel is self-contained (its own
// query + mutations, no coupling to the route's draft/Save machinery), so those land here
// without touching the rest of Settings.
//
// The updater is pull-based and quiet (D1): `idle` shows no update line at all (zero
// presence — not a "you're up to date" badge beyond the plain reassurance), and no state
// ever raises a modal, a nag, or a toast. Install happens only when the user clicks
// "Restart to update". Five states (UI_REFERENCE §4 · Updater / Settings·App rows):
// loading → skeleton; error (load) → retry; idle/checking/available/downloading/ready →
// the status line below; the updater's own `error` state → a quiet inline line + retry.
import { Button, Panel, Skeleton } from "../primitives";
import { useUpdateStatus } from "../../lib/ipc/queries";
import {
  useCheckForUpdates,
  useRestartToApplyUpdate,
} from "../../lib/ipc/mutations";
import { useAppVersion } from "../../lib/useAppVersion";
import { openExternal } from "../../lib/openExternal";
import { toast } from "../../state/toastStore";
import type { UpdateStatus } from "../../bindings/UpdateStatus";

/** The project's public repo — restated here from the NavRail footer (0.3.1 D4, #57). */
const REPO_URL = "https://github.com/nicolasestrem/screensearch-v2c";

/** The version a pending/ready update carries, or null in the states without one. */
function pendingVersion(status: UpdateStatus): string | null {
  switch (status.state) {
    case "available":
    case "downloading":
    case "ready":
      return status.version;
    default:
      return null;
  }
}

export function AppPanel() {
  const status = useUpdateStatus();
  const check = useCheckForUpdates();
  const restart = useRestartToApplyUpdate();
  const version = useAppVersion();

  // Loading → skeleton matching the final layout (no spinner-only screens, §4).
  if (status.isLoading || !status.data) {
    if (status.isError) {
      return (
        <Panel group title="App">
          <div className="flex flex-col gap-3">
            <span className="text-body text-ink-muted font-body">
              Could not load the update status.
            </span>
            <div>
              <Button variant="secondary" onClick={() => status.refetch()}>
                Retry
              </Button>
            </div>
          </div>
        </Panel>
      );
    }
    return (
      <Panel group title="App">
        <div className="flex flex-col gap-3">
          <Skeleton className="h-5 w-48" />
          <Skeleton className="h-9 w-40" />
        </div>
      </Panel>
    );
  }

  const s = status.data;
  const checking = check.isPending || s.state === "checking";
  const upVersion = pendingVersion(s);

  const runCheck = () => {
    // Quiet by contract — no success toast. A hard failure to even reach the check gets a
    // brief error toast; the persistent `error` state below carries the retry affordance.
    check.mutate(undefined, {
      onError: (e) => toast.error(`Could not check for updates: ${String(e)}`),
    });
  };

  const applyUpdate = () => {
    // On success the process hands off to the passive installer and exits — so we only
    // ever observe the error path here.
    restart.mutate(undefined, {
      onError: (e) => toast.error(`Could not install the update: ${String(e)}`),
    });
  };

  return (
    <Panel group title="App">
      <div className="flex flex-col gap-4">
        {/* Section intro — plain language, from the user's side (UI_REFERENCE §9). */}
        <p className="text-caption text-ink-muted font-body">
          ScreenSearch checks for a new version on launch. Updates never install on their
          own — you choose when to restart.
        </p>

        {/* Update status line — its shape follows the updater state (§4). */}
        <div className="flex flex-col gap-2">
          {s.state === "ready" && (
            <div className="flex flex-wrap items-center gap-3">
              <span className="text-body text-ink font-body">
                v{upVersion} available — restart to update.
              </span>
              <Button
                variant="primary"
                onClick={applyUpdate}
                disabled={restart.isPending}
              >
                {restart.isPending ? "Restarting…" : "Restart to update"}
              </Button>
            </div>
          )}

          {(s.state === "available" || s.state === "downloading") && (
            <span className="text-body text-ink-muted font-body">
              v{upVersion} available — downloading in the background…
            </span>
          )}

          {s.state === "checking" && (
            <span className="text-body text-ink-muted font-body">
              Checking for updates…
            </span>
          )}

          {s.state === "idle" && (
            <span className="text-caption text-ink-faint font-body">
              No update available — you’re on the latest version.
            </span>
          )}

          {s.state === "error" && (
            <div className="flex flex-col gap-1">
              <span className="text-body text-warn font-body">
                Couldn’t check for updates.
              </span>
              <span className="text-caption text-ink-faint font-body">
                {s.message}
              </span>
            </div>
          )}

          {/* Manual check — hidden while a ready update is waiting (restart is the action
              then) and disabled while any check/download is in flight. */}
          {s.state !== "ready" && (
            <div>
              <Button
                variant="secondary"
                onClick={runCheck}
                disabled={checking}
              >
                {checking
                  ? "Checking…"
                  : s.state === "error"
                    ? "Try again"
                    : "Check for updates"}
              </Button>
            </div>
          )}
        </div>

        {/* Version + repo link — restated here where a user looks for it (§3). Quiet: it is
            telemetry, not a primary action. Hidden with no Tauri runtime (browser dev). */}
        {version && (
          <div className="flex items-center gap-2 pt-1 border-t border-line">
            <span className="text-caption text-ink-faint font-body pt-3">
              Version
            </span>
            <a
              href={REPO_URL}
              onClick={(e) => {
                e.preventDefault();
                openExternal(REPO_URL);
              }}
              className="font-mono text-data text-ink-faint hover:text-ink transition-colors duration-fast ease-ui pt-3"
            >
              v{version}
            </a>
          </div>
        )}
      </div>
    </Panel>
  );
}
