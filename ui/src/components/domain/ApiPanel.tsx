// ApiPanel — the Settings · Local API surface (0.3.0 PR7; UI_REFERENCE §4/§5, `03 §7c`).
//
// Off by default. Enabling binds 127.0.0.1:<port> and mints a bearer token; the panel
// reveals/copies/regenerates it. A port already in use is the loud guided-change state:
// the server does not start (enabled && !running && last_error), and the panel shows the
// warning + a "pick another port" retry — mirroring the D6 hotkey-conflict pattern.
//
// Five states (UI_REFERENCE §4): loading → skeleton; error → load failed + retry;
// off (empty-off) → "API disabled" + the threat-model copy; enabling (partial) →
// "Binding…"; populated → "Listening on 127.0.0.1:<port>" + token controls.
import { useEffect, useState } from "react";

import { Button, Chip, Field, Panel, Skeleton, Toggle } from "../primitives";
import { useApiStatus } from "../../lib/ipc/queries";
import {
  useRegenerateApiToken,
  useSetApiConfig,
} from "../../lib/ipc/mutations";
import { toast } from "../../state/toastStore";

// The threat model, verbatim from `03 §7c` — shown wherever the API can be enabled.
const THREAT_MODEL =
  "Any local process holding the token can read your entire screen history — enabling this is an explicit trust decision.";

const DEFAULT_PORT = 43210;

export function ApiPanel() {
  const status = useApiStatus();
  const setConfig = useSetApiConfig();
  const regenerate = useRegenerateApiToken();

  const [portDraft, setPortDraft] = useState(String(DEFAULT_PORT));
  const [revealed, setRevealed] = useState(false);

  // Seed the port field from the loaded status once it arrives (and whenever the backend
  // reports a different port), without fighting the user mid-edit.
  const loadedPort = status.data?.port;
  useEffect(() => {
    if (loadedPort != null) setPortDraft(String(loadedPort));
  }, [loadedPort]);

  // Loading → skeleton that matches the final layout (no spinner-only screens, §4).
  if (status.isLoading || !status.data) {
    if (status.isError) {
      return (
        <Panel group title="Local API">
          <div className="flex flex-col gap-3">
            <span className="text-body text-ink-muted font-body">
              Could not load the local API status.
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
      <Panel group title="Local API">
        <div className="flex flex-col gap-3">
          <Skeleton className="h-6 w-40" />
          <Skeleton className="h-16 w-full" />
        </div>
      </Panel>
    );
  }

  const s = status.data;
  const busy = setConfig.isPending || regenerate.isPending;
  const bindFailed = s.enabled && !s.running && Boolean(s.last_error);

  const parsedPort = (): number => {
    const n = Number.parseInt(portDraft, 10);
    return Number.isFinite(n) ? n : DEFAULT_PORT;
  };

  const toggle = (next: boolean) => {
    setConfig.mutate(
      { enabled: next, port: next ? parsedPort() : null },
      {
        onSuccess: (result) => {
          if (!next) {
            toast.info("Local API disabled.");
          } else if (result.running) {
            toast.success(`Local API listening on 127.0.0.1:${result.port}.`);
          }
          // A bind failure (enabled but not running) already surfaced a warning toast
          // from the backend; the inline warning below carries the guided retry.
        },
        onError: (e) => toast.error(`Could not change the local API: ${String(e)}`),
      },
    );
  };

  const retryBind = () => {
    setConfig.mutate(
      { enabled: true, port: parsedPort() },
      {
        onSuccess: (result) => {
          if (result.running) {
            toast.success(`Local API listening on 127.0.0.1:${result.port}.`);
          }
        },
        onError: (e) => toast.error(`Could not start the local API: ${String(e)}`),
      },
    );
  };

  const copyToken = async () => {
    if (!s.token) return;
    try {
      await navigator.clipboard.writeText(s.token);
      toast.success("Token copied to the clipboard.");
    } catch {
      toast.error("Could not copy the token.");
    }
  };

  const regenerateToken = () => {
    regenerate.mutate(undefined, {
      onSuccess: () => toast.success("Generated a new token."),
      onError: (e) => toast.error(`Could not regenerate the token: ${String(e)}`),
    });
  };

  const statusChip = setConfig.isPending ? (
    <Chip tone="accent">binding…</Chip>
  ) : s.running ? (
    <Chip tone="ok">listening</Chip>
  ) : bindFailed ? (
    <Chip tone="warn">port in use</Chip>
  ) : (
    <Chip tone="neutral">disabled</Chip>
  );

  return (
    <Panel group title="Local API" action={statusChip}>
      <div className="flex flex-col gap-4">
        <Toggle
          label="Enable local HTTP API"
          checked={s.enabled}
          onChange={toggle}
          disabled={busy}
          hint="Serves your screen history to local scripts and agents over 127.0.0.1 only. Off by default."
        />

        <p className="text-caption text-warn font-body">{THREAT_MODEL}</p>

        {/* Off (empty-off): nothing else to show — the toggle + threat model is the state. */}
        {!s.enabled && (
          <p className="text-caption text-ink-faint font-body">
            API disabled — nothing is listening.
          </p>
        )}

        {/* Enabling (partial). */}
        {setConfig.isPending && (
          <p className="text-caption text-ink-muted font-body">
            Binding 127.0.0.1:{parsedPort()}…
          </p>
        )}

        {/* Port field: editable while enabling/retrying or when off. */}
        {s.enabled && (
          <Field
            label="Port"
            type="number"
            min={1024}
            max={65535}
            value={portDraft}
            onChange={(e) => setPortDraft(e.currentTarget.value)}
            disabled={busy}
            error={bindFailed ? s.last_error : null}
            hint="127.0.0.1 only (not configurable). Changing the port restarts the server."
          />
        )}

        {/* Loud guided-change: the port was in use, so offer a retry (D6 mirror). */}
        {bindFailed && (
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="secondary" onClick={retryBind} disabled={busy}>
              Retry on this port
            </Button>
            <span className="text-caption text-ink-faint">
              Pick another port above, then retry.
            </span>
          </div>
        )}

        {/* Populated: listening — show the bearer token controls. */}
        {s.running && (
          <div className="flex flex-col gap-2">
            <span className="text-body text-ink font-body">
              Listening on 127.0.0.1:{s.port}
            </span>
            <label className="text-caption text-ink-muted font-body">
              Bearer token
            </label>
            <div className="flex flex-wrap items-center gap-2">
              <code className="min-w-0 flex-1 truncate rounded-chip border border-line bg-base px-3 py-2 font-mono text-data text-ink">
                {s.token
                  ? revealed
                    ? s.token
                    : "•".repeat(24)
                  : "no token"}
              </code>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setRevealed((r) => !r)}
                aria-pressed={revealed}
              >
                {revealed ? "Hide" : "Reveal"}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={copyToken}
                disabled={!s.token}
              >
                Copy
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={regenerateToken}
                disabled={busy}
              >
                Regenerate
              </Button>
            </div>
            <span className="text-caption text-ink-faint">
              Send it as <code className="font-mono">Authorization: Bearer …</code>.
              Regenerating takes effect immediately.
            </span>
          </div>
        )}
      </div>
    </Panel>
  );
}
