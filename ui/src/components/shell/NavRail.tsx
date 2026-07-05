// NavRail (left) — the five primary destinations (UI_REFERENCE §3). Active route
// gets the accent (text + wash + a left scan-bar); the Ctrl+K hint opens the
// command palette; the footer carries a quiet version link → the GitHub repo (0.3.1 D4).
// Each link is a NavLink so the browser/router own focus + history.
// Keyboard (UI_REFERENCE §7): a roving tabindex makes the rail a single Tab stop —
// Arrow Up/Down (wrapping) and Home/End move focus between links; Enter follows one.
import { NavLink, useLocation } from "react-router-dom";
import {
  useEffect,
  useRef,
  useState,
  type ComponentType,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { cn } from "../../lib/cn";
import { useUiStore } from "../../state/uiStore";
import { useAppVersion } from "../../lib/useAppVersion";
import { openExternal } from "../../lib/openExternal";
import { UpdateIndicator } from "./UpdateIndicator";
import {
  IconDeck,
  IconRecall,
  IconTimeline,
  IconInsights,
  IconSettings,
} from "../icons";

/** The project's public repo — opened by the NavRail version link (0.3.1 D4, #57). */
const REPO_URL = "https://github.com/nicolasestrem/screensearch-v2c";

interface NavItem {
  to: string;
  label: string;
  icon: ComponentType<{ size?: number }>;
  end?: boolean;
}

const ITEMS: NavItem[] = [
  { to: "/", label: "Deck", icon: IconDeck, end: true },
  { to: "/recall", label: "Recall", icon: IconRecall },
  { to: "/timeline", label: "Timeline", icon: IconTimeline },
  { to: "/insights", label: "Insights", icon: IconInsights },
  { to: "/settings", label: "Settings", icon: IconSettings },
];

/** Index of the nav item matching the current path (Deck matches only the exact "/"). */
function activeIndexFor(pathname: string): number {
  const i = ITEMS.findIndex((it) =>
    it.end
      ? pathname === it.to
      : pathname === it.to || pathname.startsWith(`${it.to}/`),
  );
  return i < 0 ? 0 : i;
}

export function NavRail() {
  const openPalette = useUiStore((s) => s.openPalette);
  const version = useAppVersion();
  const { pathname } = useLocation();
  const linkRefs = useRef<Array<HTMLAnchorElement | null>>([]);
  // Roving tabindex: exactly one link sits in the Tab order at a time (seeded to the
  // current route); arrow keys move focus without leaving the rail.
  const [focusIndex, setFocusIndex] = useState(() => activeIndexFor(pathname));

  // Keep the tab stop on the current route when navigation happens *outside* the rail
  // (Command Palette, an in-app link, browser back/forward) — otherwise the next Tab into
  // the rail lands on a stale link. Arrow-key moves don't change the path, so they aren't
  // clobbered. Re-derives focus position only; never moves focus (no .focus() here).
  useEffect(() => {
    setFocusIndex(activeIndexFor(pathname));
  }, [pathname]);

  const focusItem = (index: number) => {
    const next = (index + ITEMS.length) % ITEMS.length; // wrap at both ends
    setFocusIndex(next);
    linkRefs.current[next]?.focus();
  };

  const onKeyDown = (
    e: ReactKeyboardEvent<HTMLAnchorElement>,
    index: number,
  ) => {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        focusItem(index + 1);
        break;
      case "ArrowUp":
        e.preventDefault();
        focusItem(index - 1);
        break;
      case "Home":
        e.preventDefault();
        focusItem(0);
        break;
      case "End":
        e.preventDefault();
        focusItem(ITEMS.length - 1);
        break;
      default:
        break;
    }
  };

  return (
    <nav
      aria-label="Primary"
      className="flex flex-col justify-between w-44 shrink-0 bg-surface border-r border-line py-4"
    >
      <ul className="flex flex-col gap-1 px-2">
        {ITEMS.map(({ to, label, icon: Icon, end }, index) => (
          <li key={to}>
            <NavLink
              to={to}
              end={end}
              ref={(el) => {
                linkRefs.current[index] = el;
              }}
              tabIndex={index === focusIndex ? 0 : -1}
              onFocus={() => setFocusIndex(index)}
              onKeyDown={(e) => onKeyDown(e, index)}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 px-3 min-h-hit-min rounded-chip border-l-2",
                  "text-body font-body transition-colors duration-fast ease-ui",
                  isActive
                    ? "bg-accent-wash text-accent border-accent"
                    : "border-transparent text-ink-muted hover:text-ink hover:bg-overlay",
                )
              }
            >
              <Icon size={18} />
              {label}
            </NavLink>
          </li>
        ))}
      </ul>

      <div className="flex flex-col gap-2 px-2">
        <button
          type="button"
          onClick={openPalette}
          className={cn(
            "flex items-center justify-between w-full gap-2 px-3 min-h-hit-min rounded-chip",
            "text-caption text-ink-muted border border-line hover:text-ink hover:border-ink-faint",
            "transition-colors duration-fast ease-ui",
          )}
        >
          <span>Command</span>
          <kbd className="font-mono text-data text-ink-faint">Ctrl+K</kbd>
        </button>
        {/* Auto-update (0.3.2 PR2, #69): a quiet presence indicator (only while an update
            exists — never a count) + the manual "Check for updates" quick-menu affordance.
            Renders nothing outside the packaged app or when no update exists (`03 §11b`). */}
        <UpdateIndicator />
        {/* Version footer link (0.3.1 D4, #57-partial): the running app version, opening
            the repo in the OS browser via the opener plugin (Tauri v2 does not open plain
            `target="_blank"` anchors). Hidden with no Tauri runtime (browser dev → null).
            Quiet by design — it is telemetry, not a primary action; the focus ring is
            global (§7). Kept an `<a>` for link semantics (role, right-click); the click is
            intercepted so it never navigates the WebView. */}
        {version && (
          <a
            href={REPO_URL}
            onClick={(e) => {
              e.preventDefault();
              openExternal(REPO_URL);
            }}
            className={cn(
              "flex items-center justify-center px-3 min-h-hit-min rounded-chip",
              "font-mono text-data text-ink-faint hover:text-ink",
              "transition-colors duration-fast ease-ui",
            )}
          >
            v{version}
          </a>
        )}
      </div>
    </nav>
  );
}
