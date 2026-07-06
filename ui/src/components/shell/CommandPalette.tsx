// CommandPalette (⌘K) — jump-to-route + quick actions (UI_REFERENCE §3). Opening
// is wired globally in AppShell; this owns query filtering, keyboard navigation
// (↑/↓ to move, Enter to run, Esc to close), and focus. Rendered only when open.
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type ComponentType,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { useNavigate } from "react-router-dom";

import { cn } from "../../lib/cn";
import { useUiStore } from "../../state/uiStore";
import { useJobStats, useSidecarStatus } from "../../lib/ipc/queries";
import {
  useCancelVision,
  useCaptureControl,
  useEnqueueVision,
  useLoadModel,
  useUnloadModel,
} from "../../lib/ipc/mutations";
import { toast } from "../../state/toastStore";
import {
  IconCapture,
  IconCpu,
  IconDeck,
  IconInsights,
  IconRecall,
  IconSettings,
  IconTag,
  IconTimeline,
} from "../icons";

interface Command {
  id: string;
  label: string;
  icon: ComponentType<{ size?: number }>;
  run: () => void;
}

export function CommandPalette() {
  const open = useUiStore((s) => s.paletteOpen);
  const close = useUiStore((s) => s.closePalette);
  const navigate = useNavigate();
  const capture = useCaptureControl();
  const sidecar = useSidecarStatus();
  const jobs = useJobStats();
  const loadModel = useLoadModel();
  const unloadModel = useUnloadModel();
  const enqueueVision = useEnqueueVision();
  const cancelVision = useCancelVision();
  // Mirror QuickActions (NavRail footer): the palette offers only the contextually valid
  // half of each toggle, so it never lists "Load" while loaded (a re-`preload()` that isn't
  // guaranteed idempotent) or "Unload"/"Stop" with nothing to act on (a noisy error/no-op
  // toast). The answer model is "loaded" only when it is the resident sidecar model.
  const answerLoaded =
    sidecar.data?.state === "ready" && sidecar.data?.lane === "answer";
  const visionActive =
    (jobs.data?.vision_pending ?? 0) + (jobs.data?.vision_running ?? 0) > 0;
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);
  // Restore focus to the opener only on *dismiss* (Esc / backdrop), not when a command
  // runs — a command navigates or acts, and the destination (e.g. Recall's autofocused
  // search input) should own focus; restoring would steal it back to the ⌘K trigger.
  const restoreFocusRef = useRef(true);
  const listboxId = "command-palette-listbox";

  const commands = useMemo<Command[]>(() => {
    const toggleCapture = (control: "start" | "stop") =>
      capture.mutate(control, {
        onSuccess: () =>
          toast.success(
            control === "start" ? "Capture started" : "Capture stopped",
          ),
        onError: (e) => toast.error(String(e)),
      });
    return [
      {
        id: "deck",
        label: "Go to Deck",
        icon: IconDeck,
        run: () => navigate("/"),
      },
      {
        id: "recall",
        label: "Go to Recall — search & ask",
        icon: IconRecall,
        run: () => navigate("/recall"),
      },
      {
        id: "timeline",
        label: "Go to Timeline",
        icon: IconTimeline,
        run: () => navigate("/timeline"),
      },
      {
        id: "insights",
        label: "Go to Insights",
        icon: IconInsights,
        run: () => navigate("/insights"),
      },
      {
        id: "settings",
        label: "Open Settings",
        icon: IconSettings,
        run: () => navigate("/settings"),
      },
      {
        id: "start",
        label: "Start capture",
        icon: IconCapture,
        run: () => toggleCapture("start"),
      },
      {
        id: "stop",
        label: "Stop capture",
        icon: IconCapture,
        run: () => toggleCapture("stop"),
      },
      // Quick actions (0.3.2 PR3, #57) — the same lifecycle actions as the tray + the
      // NavRail footer, reachable from the keyboard here too. Only the applicable half of
      // each toggle is listed, so the palette never offers a contradictory action.
      answerLoaded
        ? {
            id: "unload-answer-model",
            label: "Unload answer model",
            icon: IconCpu,
            run: () =>
              unloadModel.mutate(undefined, {
                onSuccess: () => toast.success("Answer model unloaded"),
                onError: (e) => toast.error(String(e)),
              }),
          }
        : {
            id: "load-answer-model",
            label: "Load answer model",
            icon: IconCpu,
            run: () =>
              loadModel.mutate("answer", {
                onSuccess: () => toast.success("Answer model loaded"),
                onError: (e) => toast.error(String(e)),
              }),
          },
      visionActive
        ? {
            id: "stop-vision",
            label: "Stop vision tagging",
            icon: IconTag,
            run: () =>
              cancelVision.mutate(undefined, {
                onSuccess: (n) =>
                  n > 0
                    ? toast.info(`Stopped vision tagging — cleared ${n} queued`)
                    : toast.info("Stopped vision tagging"),
                onError: (e) => toast.error(String(e)),
              }),
          }
        : {
            id: "start-vision",
            label: "Start vision tagging",
            icon: IconTag,
            run: () =>
              enqueueVision.mutate(
                { kind: "range", start: 0, end: Date.now() },
                {
                  onSuccess: (n) =>
                    n > 0
                      ? toast.success(
                          `Vision tagging started — ${n} frames queued`,
                        )
                      : toast.info("No untagged frames to tag"),
                  onError: (e) => toast.error(String(e)),
                },
              ),
          },
    ];
  }, [
    navigate,
    capture,
    answerLoaded,
    visionActive,
    loadModel,
    unloadModel,
    enqueueVision,
    cancelVision,
  ]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q
      ? commands.filter((c) => c.label.toLowerCase().includes(q))
      : commands;
  }, [commands, query]);

  // Reset and focus the input each time the palette opens, and restore focus to the
  // element that opened it (the ⌘K trigger, or wherever focus was when the hotkey
  // fired) on close — modal focus best practice, so keyboard users aren't dropped to
  // the top of the page.
  useEffect(() => {
    if (!open) return;
    openerRef.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    restoreFocusRef.current = true; // default to restore; run() opts out
    setQuery("");
    setActive(0);
    const t = setTimeout(() => inputRef.current?.focus(), 0);
    return () => {
      clearTimeout(t);
      // Only on dismiss, and only if the opener is still in the document (a command on a
      // now-unmounted page would otherwise be a detached, focus-stealing no-op).
      if (restoreFocusRef.current && openerRef.current?.isConnected) {
        openerRef.current.focus();
      }
    };
  }, [open]);

  // Keep the active index within the (possibly shrunk) filtered list. Reset to 0
  // when the list is empty so a later non-empty list starts highlighted — a bare
  // `Math.min(a, …)` latches at -1 after an ArrowDown over a zero-match query,
  // which then makes Enter run `filtered[-1]` (undefined) once matches return.
  useEffect(() => {
    setActive((a) =>
      filtered.length > 0 ? Math.min(Math.max(0, a), filtered.length - 1) : 0,
    );
  }, [filtered.length]);

  if (!open) return null;

  const run = (cmd: Command) => {
    restoreFocusRef.current = false; // the command's destination/action owns focus next
    close();
    cmd.run();
  };
  const activeOption = filtered[active]
    ? `command-option-${filtered[active].id}`
    : undefined;

  const onKeyDown = (e: ReactKeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const cmd = filtered[active];
      if (cmd) run(cmd);
    }
  };

  return (
    <div
      className="fixed inset-0 z-overlay flex items-start justify-center"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
    >
      <div
        className="absolute inset-0 bg-scrim"
        onClick={close}
        aria-hidden="true"
      />
      <div
        className="relative mt-24 w-full max-w-lg bg-overlay border border-line rounded-panel overflow-hidden"
        onKeyDown={onKeyDown}
      >
        <input
          ref={inputRef}
          type="text"
          role="combobox"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Jump to… or run a command"
          aria-label="Command palette query"
          aria-expanded="true"
          aria-autocomplete="list"
          aria-controls={listboxId}
          aria-activedescendant={activeOption}
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          spellCheck={false}
          className="w-full bg-transparent px-4 py-3 text-body text-ink placeholder:text-ink-faint border-b border-line font-body"
        />
        <ul
          id={listboxId}
          role="listbox"
          className="max-h-80 overflow-y-auto py-2"
        >
          {filtered.length === 0 ? (
            <li className="px-4 py-3 text-body text-ink-muted font-body">
              No matching command
            </li>
          ) : (
            filtered.map((c, i) => {
              const Icon = c.icon;
              return (
                <li key={c.id} role="presentation">
                  <button
                    id={`command-option-${c.id}`}
                    type="button"
                    role="option"
                    aria-selected={i === active}
                    onMouseEnter={() => setActive(i)}
                    onClick={() => run(c)}
                    className={cn(
                      "flex items-center gap-3 w-full px-4 min-h-hit-min text-left text-body font-body",
                      i === active
                        ? "bg-accent-wash text-accent"
                        : "text-ink hover:bg-overlay",
                    )}
                  >
                    <Icon size={18} />
                    {c.label}
                  </button>
                </li>
              );
            })
          )}
        </ul>
      </div>
    </div>
  );
}
