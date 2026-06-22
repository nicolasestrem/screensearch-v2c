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
  children: ReactNode;
}

export function Panel({ title, action, flush = false, className, children, ...rest }: PanelProps) {
  return (
    <section
      className={cn("bg-surface border border-line rounded-panel", className)}
      {...rest}
    >
      {(title || action) && (
        <header className="flex items-center justify-between gap-3 px-4 py-3 border-b border-line">
          {title ? <span className="eyebrow">{title}</span> : <span />}
          {action}
        </header>
      )}
      <div className={cn(!flush && "p-4")}>{children}</div>
    </section>
  );
}
