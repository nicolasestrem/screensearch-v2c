// ModelTierPicker (UI_REFERENCE §5) — a per-lane segmented control for the model
// tier (Default / Quality / Beta, `00 §E`). It owns the choice, not the persistence:
// the parent decides what a change means (Settings fires `set_model_tier` for an
// immediate hot-apply, then folds the value into the saved form). Mirrors the
// segmented range presets used on the Timeline so the instrument language is shared.
import type { ReactNode } from "react";

import type { ModelLane } from "../../bindings/ModelLane";
import type { ModelTier } from "../../bindings/ModelTier";
import { Tooltip } from "../primitives";
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

// The actual model behind each (lane, tier), so a hover/focus tooltip tells the user what
// "Default / Quality / Beta" resolve to. Mirrors the source of truth in
// `crates/inference/src/models.rs::repo_for` (and `specs/MODEL_REGISTRY.md`) — display
// names only (repo org + `-GGUF` suffix dropped); keep in sync if the registry changes.
const MODEL_NAMES: Record<ModelLane, Record<ModelTier, string>> = {
  vision: {
    default: "Qwen3-VL-4B-Instruct",
    quality: "Qwen3-VL-8B-Instruct",
    beta: "Qwen3.5-9B-VLM",
  },
  answer: {
    default: "Ministral-3-3B-Reasoning",
    quality: "Qwen3-4B-Thinking",
    beta: "NVIDIA-Nemotron-3-Nano-4B",
  },
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
          <Tooltip key={t.value} label={MODEL_NAMES[lane][t.value]} side="bottom">
            <button
              type="button"
              aria-pressed={value === t.value}
              aria-label={`${t.label} — ${MODEL_NAMES[lane][t.value]}`}
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
          </Tooltip>
        ))}
      </div>
      {hint && <span className="text-caption text-ink-faint">{hint}</span>}
    </div>
  );
}
