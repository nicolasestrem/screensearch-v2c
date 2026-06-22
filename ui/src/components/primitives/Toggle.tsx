// Toggle — an accessible on/off switch (role="switch"). Used for the enrichment
// schedule opt-ins and privacy flags. Keyboard: Space/Enter toggles (native button).
import { useId, type ReactNode } from "react";
import { cn } from "../../lib/cn";

export interface ToggleProps {
  label: string;
  checked: boolean;
  onChange: (next: boolean) => void;
  hint?: ReactNode;
  disabled?: boolean;
}

export function Toggle({ label, checked, onChange, hint, disabled = false }: ToggleProps) {
  const id = useId();
  const hintId = `${id}-hint`;

  return (
    <div className="flex items-start justify-between gap-4">
      <div className="flex flex-col gap-1">
        <label id={id} className="text-body text-ink font-body">
          {label}
        </label>
        {hint && (
          <span id={hintId} className="text-caption text-ink-faint">
            {hint}
          </span>
        )}
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-labelledby={id}
        aria-describedby={hint ? hintId : undefined}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={cn(
          "relative shrink-0 w-12 h-hit-min rounded-chip border transition-colors duration-fast ease-ui",
          "disabled:opacity-50 disabled:pointer-events-none",
          checked ? "bg-accent-wash border-accent" : "bg-base border-line",
        )}
      >
        <span
          aria-hidden="true"
          className={cn(
            "absolute top-1 w-6 h-6 rounded-chip transition-all duration-fast ease-ui",
            checked ? "left-6 bg-accent" : "left-1 bg-ink-faint",
          )}
        />
      </button>
    </div>
  );
}
