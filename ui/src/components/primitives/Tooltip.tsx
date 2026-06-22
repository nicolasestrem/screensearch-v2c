// Tooltip — a lightweight label shown on hover and keyboard focus. Appears above
// the wrapped element; uses focus-within so keyboard users get it too. role=
// "tooltip" for assistive tech. Reduced-motion users see it appear instantly.
import { useState, type ReactNode } from "react";
import { cn } from "../../lib/cn";

export interface TooltipProps {
  label: ReactNode;
  /** Where to place the tooltip relative to the trigger. */
  side?: "top" | "bottom";
  children: ReactNode;
}

export function Tooltip({ label, side = "top", children }: TooltipProps) {
  const [open, setOpen] = useState(false);

  return (
    <span
      className="relative inline-flex"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      onFocus={() => setOpen(true)}
      onBlur={() => setOpen(false)}
    >
      {children}
      {open && (
        <span
          role="tooltip"
          className={cn(
            "absolute left-1/2 -translate-x-1/2 z-overlay whitespace-nowrap pointer-events-none",
            "bg-overlay border border-line rounded-chip px-2 py-1",
            "text-caption text-ink font-body",
            side === "top" ? "bottom-full mb-2" : "top-full mt-2",
          )}
        >
          {label}
        </span>
      )}
    </span>
  );
}
