// IconButton — a square, icon-only action. `label` is mandatory and becomes the
// accessible name + tooltip title (UI_REFERENCE §7: every control is labelled).
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cn } from "../../lib/cn";

export interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label"> {
  label: string;
  active?: boolean;
  children: ReactNode;
}

export function IconButton({
  label,
  active = false,
  className,
  type = "button",
  children,
  ...rest
}: IconButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      aria-pressed={active}
      title={label}
      className={cn(
        "inline-flex items-center justify-center rounded-chip w-hit-min h-hit-min",
        "transition-colors duration-fast ease-ui",
        "disabled:opacity-50 disabled:pointer-events-none",
        active ? "bg-accent-wash text-accent" : "text-ink-muted hover:text-ink hover:bg-overlay",
        className,
      )}
      {...rest}
    >
      {children}
    </button>
  );
}
