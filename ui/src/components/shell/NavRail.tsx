// NavRail (left) — the five primary destinations (UI_REFERENCE §3). Active route
// gets the accent (text + wash + a left scan-bar); the Ctrl+K hint opens the
// command palette. Each link is a NavLink so the browser/router own focus + history.
// Keyboard (UI_REFERENCE §7): a roving tabindex makes the rail a single Tab stop —
// Arrow Up/Down (wrapping) and Home/End move focus between links; Enter follows one.
import { NavLink, useLocation } from "react-router-dom";
import { useEffect, useRef, useState, type ComponentType, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { cn } from "../../lib/cn";
import { useUiStore } from "../../state/uiStore";
import {
  IconDeck,
  IconRecall,
  IconTimeline,
  IconInsights,
  IconSettings,
} from "../icons";

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
    it.end ? pathname === it.to : pathname === it.to || pathname.startsWith(`${it.to}/`),
  );
  return i < 0 ? 0 : i;
}

export function NavRail() {
  const openPalette = useUiStore((s) => s.openPalette);
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

  const onKeyDown = (e: ReactKeyboardEvent<HTMLAnchorElement>, index: number) => {
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

      <div className="px-2">
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
      </div>
    </nav>
  );
}
