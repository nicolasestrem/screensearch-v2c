// Toast — transient client-side notification. `Toast` renders one item (and owns
// its auto-dismiss timer); `ToastViewport` subscribes to the toast store and
// stacks them in the toast z-layer. Toasts are driven by command success/failure
// (the backend never emits a `toast` event — plan decision).
import { useEffect } from "react";
import { cn } from "../../lib/cn";
import { useToastStore, type ToastItem } from "../../state/toastStore";
import type { ToastLevel } from "../../bindings/ToastLevel";

/** How long a toast stays before auto-dismissing (errors linger longer). */
const DURATIONS: Record<ToastLevel, number> = {
  info: 4000,
  success: 4000,
  warning: 6000,
  error: 8000,
};

const ACCENT: Record<ToastLevel, string> = {
  info: "border-l-accent",
  success: "border-l-ok",
  warning: "border-l-warn",
  error: "border-l-danger",
};

export interface ToastProps {
  toast: ToastItem;
  onClose: (id: number) => void;
}

export function Toast({ toast, onClose }: ToastProps) {
  const { id, level, message } = toast;

  useEffect(() => {
    const timer = setTimeout(() => onClose(id), DURATIONS[level]);
    return () => clearTimeout(timer);
  }, [id, level, onClose]);

  return (
    <div
      className={cn(
        "flex items-start gap-3 bg-overlay border border-line border-l-2 rounded-panel",
        "px-4 py-3 max-w-sm",
        ACCENT[level],
      )}
    >
      <span className="text-body text-ink font-body flex-1">{message}</span>
      <button
        type="button"
        aria-label="Dismiss"
        onClick={() => onClose(id)}
        className="text-ink-faint hover:text-ink transition-colors duration-fast"
      >
        ✕
      </button>
    </div>
  );
}

export function ToastViewport() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  if (toasts.length === 0) return null;

  return (
    <div
      aria-live="polite"
      className="fixed bottom-4 right-4 z-toast flex flex-col gap-2 items-end"
    >
      {toasts.map((t) => (
        <Toast key={t.id} toast={t} onClose={dismiss} />
      ))}
    </div>
  );
}
