// EmptyState — an *invitation to act*, never decorative mood (UI_REFERENCE §4/§9).
// Title states the situation plainly; description tells the user what to do; the
// optional action is the one primary thing to do next.
import type { ReactNode } from "react";

export interface EmptyStateProps {
  /** Inline SVG icon (no web fonts). */
  icon?: ReactNode;
  title: string;
  description?: ReactNode;
  /** The single primary action (e.g. a "Start capture" Button). */
  action?: ReactNode;
}

export function EmptyState({ icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center text-center gap-3 px-6 py-12">
      {icon && <div className="text-ink-faint">{icon}</div>}
      <h2 className="font-display uppercase tracking-eyebrow text-subtitle text-ink">{title}</h2>
      {description && (
        <p className="text-body text-ink-muted max-w-prose font-body">{description}</p>
      )}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}
