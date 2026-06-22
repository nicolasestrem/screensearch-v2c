// Select — a labelled native <select> styled to the console palette. Native so it
// inherits OS keyboard behavior and accessibility for free.
import { useId, type SelectHTMLAttributes, type ReactNode } from "react";
import { cn } from "../../lib/cn";

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectProps extends Omit<SelectHTMLAttributes<HTMLSelectElement>, "id"> {
  label: string;
  options: SelectOption[];
  hint?: ReactNode;
}

export function Select({ label, options, hint, className, ...rest }: SelectProps) {
  const id = useId();
  const hintId = `${id}-hint`;

  return (
    <div className="flex flex-col gap-2">
      <label htmlFor={id} className="text-caption text-ink-muted font-body">
        {label}
      </label>
      <select
        id={id}
        aria-describedby={hint ? hintId : undefined}
        className={cn(
          "bg-base border border-line rounded-chip px-3 min-h-hit-min text-body text-ink font-body",
          "transition-colors duration-fast ease-ui focus:border-accent",
          className,
        )}
        {...rest}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      {hint && (
        <span id={hintId} className="text-caption text-ink-faint">
          {hint}
        </span>
      )}
    </div>
  );
}
