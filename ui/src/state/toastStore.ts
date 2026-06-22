// Client-side toast queue. Toasts are driven by command success/failure in the UI
// (the backend never emits a `toast` event — plan decision); the ts-rs `Toast`
// type is kept for forward-compat but not used as a transport here.
import { create } from "zustand";
import type { ToastLevel } from "../bindings/ToastLevel";

export interface ToastItem {
  id: number;
  level: ToastLevel;
  message: string;
}

interface ToastState {
  toasts: ToastItem[];
  /** Enqueue a toast; returns its id (for manual dismissal). */
  push: (level: ToastLevel, message: string) => number;
  dismiss: (id: number) => void;
}

// Monotonic id source. Module-level counter (not Date/random) so it is
// deterministic and SSR/test-safe.
let nextId = 1;

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],
  push: (level, message) => {
    const id = nextId++;
    set((s) => ({ toasts: [...s.toasts, { id, level, message }] }));
    return id;
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

/**
 * Convenience facade for non-component code (mutation onSuccess/onError) to fire
 * a toast without a hook. Mirrors the `ToastLevel` variants.
 */
export const toast = {
  info: (message: string) => useToastStore.getState().push("info", message),
  success: (message: string) => useToastStore.getState().push("success", message),
  warning: (message: string) => useToastStore.getState().push("warning", message),
  error: (message: string) => useToastStore.getState().push("error", message),
};
