// Field — a labelled text/number input. Associates label, hint, and error with
// the control via ids (aria-describedby / aria-invalid). Used for capture/privacy
// settings in M5.
import { useId, type InputHTMLAttributes, type ReactNode } from "react";
import { cn } from "../../lib/cn";

export interface FieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "id"> {
  label: string;
  /** Helper text below the input (e.g. "applies on next capture start"). */
  hint?: ReactNode;
  /** Error message; sets aria-invalid and the danger border. */
  error?: string | null;
}

export function Field({ label, hint, error, className, ...rest }: FieldProps) {
  const id = useId();
  const hintId = `${id}-hint`;
  const errorId = `${id}-error`;

  return (
    <div className="flex flex-col gap-2">
      <label htmlFor={id} className="text-caption text-ink-muted font-body">
        {label}
      </label>
      <input
        id={id}
        aria-invalid={error ? true : undefined}
        aria-describedby={cn(hint ? hintId : null, error ? errorId : null) || undefined}
        className={cn(
          "bg-base border rounded-chip px-3 min-h-hit-min text-body text-ink font-body",
          "transition-colors duration-fast ease-ui",
          "placeholder:text-ink-faint",
          error ? "border-danger" : "border-line focus:border-accent",
          className,
        )}
        {...rest}
      />
      {hint && !error && (
        <span id={hintId} className="text-caption text-ink-faint">
          {hint}
        </span>
      )}
      {error && (
        <span id={errorId} className="text-caption text-danger">
          {error}
        </span>
      )}
    </div>
  );
}
