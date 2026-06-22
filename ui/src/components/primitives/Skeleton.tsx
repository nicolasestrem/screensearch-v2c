// Skeleton — a layout-reserving placeholder for loading states. Pulses subtly;
// the pulse is killed under prefers-reduced-motion (globals.css), so it degrades
// to a static block. Skeletons must match the final layout (UI_REFERENCE §4/§8:
// no spinner-only screens, no layout shift on data arrival).
import type { HTMLAttributes } from "react";
import { cn } from "../../lib/cn";

export interface SkeletonProps extends HTMLAttributes<HTMLDivElement> {
  /** Round the block fully (e.g. avatar / dot placeholders). */
  circle?: boolean;
}

export function Skeleton({ circle = false, className, ...rest }: SkeletonProps) {
  return (
    <div
      aria-hidden="true"
      className={cn("animate-pulse bg-overlay", circle ? "rounded-full" : "rounded-chip", className)}
      {...rest}
    />
  );
}
