// Button — the one action primitive. Industrial instrument styling: condensed
// display face, uppercase, tracked. Tokens only; ≥32px hit target; visible focus
// comes from the global :focus-visible rule (UI_REFERENCE §7).
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cn } from "../../lib/cn";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md";

const VARIANTS: Record<ButtonVariant, string> = {
  // `text-base` here is the near-black background token used as a knockout color
  // on the bright accent (AA-verified ~6.6:1), not a font size.
  primary: "bg-accent text-base hover:opacity-90",
  secondary: "bg-surface text-ink border border-line hover:border-ink-faint",
  ghost: "bg-transparent text-ink-muted hover:text-ink hover:bg-overlay",
  danger: "bg-transparent text-danger border border-danger hover:bg-danger hover:text-base",
};

const SIZES: Record<ButtonSize, string> = {
  sm: "px-2 min-h-hit-min text-caption",
  md: "px-4 min-h-hit-min text-body",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  leadingIcon?: ReactNode;
}

export function Button({
  variant = "secondary",
  size = "md",
  leadingIcon,
  className,
  type = "button",
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-chip font-display uppercase tracking-eyebrow font-semibold",
        "transition-colors duration-fast ease-ui select-none",
        "disabled:opacity-50 disabled:pointer-events-none",
        VARIANTS[variant],
        SIZES[size],
        className,
      )}
      {...rest}
    >
      {leadingIcon}
      {children}
    </button>
  );
}
