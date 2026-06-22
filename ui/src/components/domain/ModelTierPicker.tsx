// ModelTierPicker (UI_REFERENCE §5) — a per-lane segmented control for the model
// tier (Default / Quality / Beta, `00 §E`). It owns the choice, not the persistence:
// the parent decides what a change means (Settings fires `set_model_tier` for an
// immediate hot-apply, then folds the value into the saved form). Mirrors the
// segmented range presets used on the Timeline so the instrument language is shared.
import type { ReactNode } from "react";

import type { ModelLane } from "../../bindings/ModelLane";
import type { ModelTier } from "../../bindings/ModelTier";
import { cn } from "../../lib/cn";

const TIERS: { value: ModelTier; label: string }[] = [
  { value: "default", label: "Default" },
  { value: "quality", label: "Quality" },
  { value: "beta", label: "Beta" },
];

const LANE_LABEL: Record<ModelLane, string> = {
  vision: "Vision model",
  answer: "Answer model",
};

export interface ModelTierPickerProps {
  lane: ModelLane;
  value: ModelTier;
  onChange: (tier: ModelTier) => void;
  /** Helper text below the control (e.g. "Applies now"). */
  hint?: ReactNode;
  disabled?: boolean;
}

export function ModelTierPicker({ lane, value, onChange, hint, disabled = false }: ModelTierPickerProps) {
  const label = LANE_LABEL[lane];
  return (
    <div className="flex flex-col gap-2">
      <span className="text-caption text-ink-muted font-body">{label}</span>
      <div role="group" aria-label={`${label} tier`} className="flex gap-1">
        {TIERS.map((t) => (
          <button
            key={t.value}
            type="button"
            aria-pressed={value === t.value}
            disabled={disabled}
            onClick={() => onChange(t.value)}
            className={cn(
              "inline-flex items-center rounded-chip px-3 min-h-hit-min font-display uppercase tracking-eyebrow text-caption font-semibold",
              "transition-colors duration-fast ease-ui disabled:opacity-50 disabled:pointer-events-none",
              value === t.value
                ? "bg-accent-wash text-accent"
                : "text-ink-muted hover:text-ink hover:bg-overlay",
            )}
          >
            {t.label}
          </button>
        ))}
      </div>
      {hint && <span className="text-caption text-ink-faint">{hint}</span>}
    </div>
  );
}
