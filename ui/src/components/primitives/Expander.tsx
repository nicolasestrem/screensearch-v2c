// Expander — a Panel-shaped disclosure for a collapsed-by-default settings group
// (0.3.2 PR5, the Settings Advanced tier — UI_REFERENCE §3, D6). The whole header row
// is the disclosure button (`aria-expanded` + `aria-controls`); the body is a labelled
// region, so the group keeps the form-group semantics Settings panels get from
// `Panel group`. The intro sentence stays visible while collapsed — it is what tells a
// user whether the group is worth opening. Open state belongs to the caller (per-session
// via the ui store; no persistence machinery). Visible focus comes from the global
// :focus-visible rule (§7); the chevron transition is zeroed by the global
// prefers-reduced-motion rule (§1/§7).
import { useId, type ReactNode } from "react";

import { IconChevronRight } from "../icons";
import { cn } from "../../lib/cn";

export interface ExpanderProps {
  /** Eyebrow label shown in the header (display face, uppercase). */
  title: string;
  /** One plain-language sentence on what the group is for (UI_REFERENCE §9). */
  intro?: string;
  /** Whether the body is shown. Controlled by the caller. */
  open: boolean;
  onToggle: () => void;
  children: ReactNode;
}

export function Expander({ title, intro, open, onToggle, children }: ExpanderProps) {
  const id = useId();
  const titleId = `${id}-title`;
  const bodyId = `${id}-body`;
  return (
    <section className="bg-surface border border-line rounded-panel">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={bodyId}
        onClick={onToggle}
        className="flex w-full items-center justify-between gap-3 px-4 py-3 min-h-hit-min text-left"
      >
        <span className="flex flex-col gap-0.5">
          <span id={titleId} className="eyebrow">
            {title}
          </span>
          {intro && (
            <span className="text-caption text-ink-muted font-body">{intro}</span>
          )}
        </span>
        <IconChevronRight
          size={16}
          className={cn(
            "shrink-0 text-ink-muted transition-transform duration-fast ease-ui",
            open && "rotate-90",
          )}
        />
      </button>
      {open && (
        <div
          id={bodyId}
          role="region"
          aria-labelledby={titleId}
          className="border-t border-line p-4"
        >
          {children}
        </div>
      )}
    </section>
  );
}
