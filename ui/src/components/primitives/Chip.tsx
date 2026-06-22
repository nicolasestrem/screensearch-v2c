// Chip — a small data pill (status, count, label). Mono by default so numeric
// data stays tabular. `tone` colors functional status without adding a hue.
import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "../../lib/cn";

export type ChipTone = "neutral" | "accent" | "ok" | "warn" | "danger";

const TONES: Record<ChipTone, string> = {
  neutral: "text-ink-muted border-line",
  accent: "text-accent border-accent",
  ok: "text-ok border-ok",
  warn: "text-warn border-warn",
  danger: "text-danger border-danger",
};

export interface ChipProps extends HTMLAttributes<HTMLSpanElement> {
  tone?: ChipTone;
  /** Render a leading status dot in the tone color. */
  dot?: boolean;
  children: ReactNode;
}

export function Chip({ tone = "neutral", dot = false, className, children, ...rest }: ChipProps) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-2 rounded-chip border px-2 py-1",
        "font-mono text-data leading-none whitespace-nowrap",
        TONES[tone],
        className,
      )}
      {...rest}
    >
      {dot && <span className="w-2 h-2 rounded-full bg-current" aria-hidden="true" />}
      {children}
    </span>
  );
}
