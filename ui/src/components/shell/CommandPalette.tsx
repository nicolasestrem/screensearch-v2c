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
import { useCaptureControl } from "../../lib/ipc/mutations";
import { toast } from "../../state/toastStore";
import {
  IconCapture,
  IconDeck,
  IconInsights,
  IconRecall,
  IconSettings,
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
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const commands = useMemo<Command[]>(() => {
    const toggleCapture = (control: "start" | "stop") =>
      capture.mutate(control, {
        onSuccess: () => toast.success(control === "start" ? "Capture started" : "Capture stopped"),
        onError: (e) => toast.error(String(e)),
      });
    return [
      { id: "deck", label: "Go to Deck", icon: IconDeck, run: () => navigate("/") },
      { id: "recall", label: "Go to Recall — search & ask", icon: IconRecall, run: () => navigate("/recall") },
      { id: "timeline", label: "Go to Timeline", icon: IconTimeline, run: () => navigate("/timeline") },
      { id: "insights", label: "Go to Insights", icon: IconInsights, run: () => navigate("/insights") },
      { id: "settings", label: "Open Settings", icon: IconSettings, run: () => navigate("/settings") },
      { id: "start", label: "Start capture", icon: IconCapture, run: () => toggleCapture("start") },
      { id: "stop", label: "Stop capture", icon: IconCapture, run: () => toggleCapture("stop") },
    ];
  }, [navigate, capture]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? commands.filter((c) => c.label.toLowerCase().includes(q)) : commands;
  }, [commands, query]);

  // Reset and focus the input each time the palette opens.
  useEffect(() => {
    if (!open) return;
    setQuery("");
    setActive(0);
    const t = setTimeout(() => inputRef.current?.focus(), 0);
    return () => clearTimeout(t);
  }, [open]);

  // Keep the active index within the (possibly shrunk) filtered list. Reset to 0
  // when the list is empty so a later non-empty list starts highlighted — a bare
  // `Math.min(a, …)` latches at -1 after an ArrowDown over a zero-match query,
  // which then makes Enter run `filtered[-1]` (undefined) once matches return.
  useEffect(() => {
    setActive((a) => (filtered.length > 0 ? Math.min(Math.max(0, a), filtered.length - 1) : 0));
  }, [filtered.length]);

  if (!open) return null;

  const run = (cmd: Command) => {
    close();
    cmd.run();
  };

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
      <div className="absolute inset-0 bg-scrim" onClick={close} aria-hidden="true" />
      <div
        className="relative mt-24 w-full max-w-lg bg-overlay border border-line rounded-panel overflow-hidden"
        onKeyDown={onKeyDown}
      >
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Jump to… or run a command"
          aria-label="Command palette query"
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          spellCheck={false}
          className="w-full bg-transparent px-4 py-3 text-body text-ink placeholder:text-ink-faint border-b border-line font-body"
        />
        <ul className="max-h-80 overflow-y-auto py-2">
          {filtered.length === 0 ? (
            <li className="px-4 py-3 text-body text-ink-muted font-body">No matching command</li>
          ) : (
            filtered.map((c, i) => {
              const Icon = c.icon;
              return (
                <li key={c.id}>
                  <button
                    type="button"
                    onMouseEnter={() => setActive(i)}
                    onClick={() => run(c)}
                    className={cn(
                      "flex items-center gap-3 w-full px-4 min-h-hit-min text-left text-body font-body",
                      i === active ? "bg-accent-wash text-accent" : "text-ink hover:bg-overlay",
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
