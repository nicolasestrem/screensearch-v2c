// Write-side hooks. Each wraps a command in a TanStack mutation and reconciles
// the affected query cache on success. Toasts are client-side (UI_REFERENCE §6 /
// plan): callers fire them from onSuccess/onError, the backend never emits `toast`.
import { useMutation, useQueryClient } from "@tanstack/react-query";

import * as cmd from "./commands";
import { queryKeys } from "./queryKeys";
import type { CaptureControl } from "../../bindings/CaptureControl";
import type { VisionTarget } from "../../bindings/VisionTarget";
import type { Settings } from "../../bindings/Settings";
import type { SetModelTier } from "../../bindings/SetModelTier";

/** Start/stop capture; readiness refetches so the StatusRail flips immediately. */
export function useCaptureControl() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (control: CaptureControl) => cmd.captureControl(control),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.readiness });
    },
  });
}

/** Enqueue on-demand vision tagging for a frame or range. */
export function useEnqueueVision() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (target: VisionTarget) => cmd.enqueueVision(target),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.jobStats });
    },
  });
}

/**
 * Persist settings. Optimistic: the cache is set to the submitted value up front
 * and reconciled with a refetch afterwards (UI_REFERENCE §4 Settings row).
 */
export function useSetSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (settings: Settings) => cmd.setSettings(settings),
    onMutate: async (settings: Settings) => {
      await qc.cancelQueries({ queryKey: queryKeys.settings });
      const previous = qc.getQueryData<Settings>(queryKeys.settings);
      qc.setQueryData(queryKeys.settings, settings);
      return { previous };
    },
    onError: (_err, _settings, ctx) => {
      if (ctx?.previous) qc.setQueryData(queryKeys.settings, ctx.previous);
    },
    onSettled: () => {
      qc.invalidateQueries({ queryKey: queryKeys.settings });
    },
  });
}

/** Change a single lane's model tier (used by the tier picker for immediacy). */
export function useSetModelTier() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (request: SetModelTier) => cmd.setModelTier(request),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.settings });
    },
  });
}
