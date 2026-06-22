// ErrorState — explains what happened and how to fix it (UI_REFERENCE §9: never
// apologize, never vague). Offers a single retry when the caller can recover.
import type { ReactNode } from "react";
import { Button } from "./Button";

export interface ErrorStateProps {
  icon?: ReactNode;
  /** What failed, plainly (e.g. "Couldn't reach the kernel"). */
  title: string;
  /** Usually the underlying error message or a remedy. */
  message?: ReactNode;
  /** Retry handler; renders a "Try again" button when present. */
  onRetry?: () => void;
  retryLabel?: string;
}

export function ErrorState({ icon, title, message, onRetry, retryLabel = "Try again" }: ErrorStateProps) {
  return (
    <div
      role="alert"
      className="flex flex-col items-center justify-center text-center gap-3 px-6 py-12"
    >
      {icon && <div className="text-danger">{icon}</div>}
      <h2 className="font-display uppercase tracking-eyebrow text-subtitle text-ink">{title}</h2>
      {message && (
        <p className="text-body text-ink-muted max-w-prose font-body break-words">{message}</p>
      )}
      {onRetry && (
        <Button variant="secondary" onClick={onRetry} className="mt-2">
          {retryLabel}
        </Button>
      )}
    </div>
  );
}
