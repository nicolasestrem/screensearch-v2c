// Panel — a surface container (card). Optional eyebrow title and a right-aligned
// action slot. The single elevation step is surface color + a hairline (dark UI;
// minimal shadow per UI_REFERENCE §1).
import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "../../lib/cn";

export interface PanelProps extends HTMLAttributes<HTMLElement> {
  /** Eyebrow label shown in the panel header (display face, uppercase). */
  title?: string;
  /** Right-aligned header content (e.g. a Button or Chip). */
  action?: ReactNode;
  /** Removes inner padding (e.g. for an edge-to-edge ribbon). */
  flush?: boolean;
  /**
   * Expose the panel as an accessible form group: sets `role="group"` +
   * `aria-labelledby` (pointing at the title) so screen readers announce the title
   * when navigating the controls inside — the ARIA equivalent of `<fieldset>`/
   * `<legend>`, without altering the card layout. Opt-in (used by Settings); requires
   * a `title`. Non-form panels stay plain `<section>`s.
   */
  group?: boolean;
  children: ReactNode;
}

export function Panel({
  title,
  action,
  flush = false,
  group = false,
  className,
  children,
  ...rest
}: PanelProps) {
  // A stable id from the title so the group can be labelled by it (only when grouping).
  const titleId =
    group && title
      ? `panel-${title
          .toLowerCase()
          .replace(/[^a-z0-9]+/g, "-")
          .replace(/^-|-$/g, "")}`
      : undefined;
  return (
    <section
      className={cn("bg-surface border border-line rounded-panel", className)}
      role={titleId ? "group" : undefined}
      aria-labelledby={titleId}
      {...rest}
    >
      {(title || action) && (
        <header className="flex items-center justify-between gap-3 px-4 py-3 min-h-12 border-b border-line">
          {title ? (
            <span id={titleId} className="eyebrow">
              {title}
            </span>
          ) : (
            <span />
          )}
          {action}
        </header>
      )}
      <div className={cn(!flush && "p-4")}>{children}</div>
    </section>
  );
}
