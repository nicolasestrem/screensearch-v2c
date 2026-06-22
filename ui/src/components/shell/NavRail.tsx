// NavRail (left) — the five primary destinations (UI_REFERENCE §3). Active route
// gets the accent (text + wash + a left scan-bar); the ⌘K hint opens the command
// palette. Each link is a NavLink so the browser/router own focus + history.
import { NavLink } from "react-router-dom";
import type { ComponentType } from "react";
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

export function NavRail() {
  const openPalette = useUiStore((s) => s.openPalette);

  return (
    <nav
      aria-label="Primary"
      className="flex flex-col justify-between w-44 shrink-0 bg-surface border-r border-line py-4"
    >
      <ul className="flex flex-col gap-1 px-2">
        {ITEMS.map(({ to, label, icon: Icon, end }) => (
          <li key={to}>
            <NavLink
              to={to}
              end={end}
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
