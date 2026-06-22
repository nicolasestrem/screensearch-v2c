// Settings (/settings) — capture, storage/retention, model tiers, the enrichment
// schedule, privacy (UI_REFERENCE §3/§4). One editable draft of the typed `Settings`
// binding; Save persists the whole draft (optimistic + reconcile via useSetSettings).
// Model tiers additionally hot-apply the moment they change (useSetModelTier), so the
// running providers switch without waiting for Save. Every field labels *when* it
// takes effect — tiers now, the answer thinking flag on the next question, capture/
// storage/privacy on the next capture start, enrichment/sidecar on restart — matching
// the backend's honest apply policy (no fictional live reconfiguration, `03 §8`).
//
// States (§4): loading → skeleton; error → load failed + retry; partial → models
// still downloading (noted in the Models panel); populated → the form. Settings has
// no empty state. A failed Save keeps the form and explains via a toast.
import { useEffect, useState, type ChangeEvent } from "react";

import { Button, Chip, Field, Panel, Skeleton, Toggle, ErrorState } from "../components/primitives";
import { ModelTierPicker, RetentionControl, ScheduleControl } from "../components/domain";
import { useReadiness, useSettings } from "../lib/ipc/queries";
import { useSetModelTier, useSetSettings } from "../lib/ipc/mutations";
import { toast } from "../state/toastStore";
import type { Settings } from "../bindings/Settings";
import type { ModelLane } from "../bindings/ModelLane";
import type { ModelTier } from "../bindings/ModelTier";

const TIER_LABEL: Record<ModelTier, string> = {
  default: "Default",
  quality: "Quality",
  beta: "Beta",
};

// When each setting takes effect (the backend applies them at different points).
const APPLY_NOW = "Applies now.";
const APPLY_ASK = "Applies to your next question.";
const APPLY_CAPTURE = "Applies on the next capture start.";
const APPLY_RESTART = "Applies on restart.";

/** Parse a comma-separated list of non-negative monitor indices; empty → []. */
function parseIntList(raw: string): number[] {
  return raw
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0)
    .map(Number)
    .filter((n) => Number.isFinite(n) && n >= 0)
    .map((n) => Math.round(n));
}

/** Parse a comma-separated list of names, trimmed; empty entries dropped. */
function parseStrList(raw: string): string[] {
  return raw
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/**
 * Clamp/round the numeric settings into valid ranges before they're persisted. Field
 * `min`/`max` are advisory — a user can type or paste out of range — and the integer
 * Rust fields reject a JSON float, so the form sanitises on save: every integer field
 * is rounded and clamped, and `capture_diff_threshold` (the only float) is clamped to
 * [0, 1] (a normalised frame-difference can never exceed 1, so a larger value would
 * wedge the capture diff-gate). Returns a new object; never mutates the input.
 */
function sanitizeSettings(s: Settings): Settings {
  const clampInt = (v: number, lo: number, hi: number) =>
    Math.min(hi, Math.max(lo, Math.round(Number.isFinite(v) ? v : lo)));
  const clampNum = (v: number, lo: number, hi: number) =>
    Math.min(hi, Math.max(lo, Number.isFinite(v) ? v : lo));
  return {
    ...s,
    capture_interval_ms: clampInt(s.capture_interval_ms, 250, 3_600_000),
    capture_diff_threshold: clampNum(s.capture_diff_threshold, 0, 1),
    storage_jpeg_quality: clampInt(s.storage_jpeg_quality, 1, 100),
    storage_max_width: clampInt(s.storage_max_width, 320, 7680),
    storage_retention_days: clampInt(s.storage_retention_days, 0, 3650),
    enrich_worker_concurrency: clampInt(s.enrich_worker_concurrency, 1, 16),
    enrich_vision_timer_interval_ms: clampInt(s.enrich_vision_timer_interval_ms, 60_000, 86_400_000),
    enrich_vision_idle_secs: clampInt(s.enrich_vision_idle_secs, 60, 86_400),
    sidecar_idle_ttl_secs: clampInt(s.sidecar_idle_ttl_secs, 0, 86_400),
    sidecar_ngl: clampInt(s.sidecar_ngl, 0, 999),
  };
}

function SettingsSkeleton() {
  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4 p-6">
      <Skeleton className="h-12 w-full" />
      <Skeleton className="h-48 w-full" />
      <Skeleton className="h-48 w-full" />
      <Skeleton className="h-40 w-full" />
    </div>
  );
}

export function Component() {
  const settings = useSettings();
  const readiness = useReadiness();
  const setSettings = useSetSettings();
  const setTier = useSetModelTier();

  // The form's working copy and the last-saved snapshot it's diffed against. The two
  // free-text list fields keep their own raw buffers so typing (trailing commas etc.)
  // isn't fought by re-serialising the parsed array back into the input.
  const [draft, setDraft] = useState<Settings | null>(null);
  const [baseline, setBaseline] = useState<Settings | null>(null);
  const [monitorsText, setMonitorsText] = useState("");
  const [appsText, setAppsText] = useState("");

  // Keep the saved-snapshot in sync with the backend on every refetch so the `dirty`
  // diff stays accurate (e.g. after a tier hot-apply invalidation, or an optimistic
  // save settling). The editable draft is seeded only once — a later refetch must not
  // clobber in-progress edits.
  useEffect(() => {
    if (!settings.data) return;
    setBaseline(settings.data);
    if (draft === null) {
      setDraft(settings.data);
      setMonitorsText(settings.data.capture_monitors.join(", "));
      setAppsText(settings.data.privacy_excluded_apps.join(", "));
    }
  }, [settings.data, draft]);

  if (settings.isError && !draft) {
    return (
      <div className="p-6">
        <ErrorState
          title="Couldn't load settings"
          message={String(settings.error)}
          onRetry={() => settings.refetch()}
        />
      </div>
    );
  }

  if (!draft || !baseline) return <SettingsSkeleton />;

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setDraft((d) => (d ? { ...d, [key]: value } : d));

  const patch = (p: Partial<Settings>) => setDraft((d) => (d ? { ...d, ...p } : d));

  // Numeric inputs: a cleared field yields NaN — fall back to 0 (a transient value the
  // user types over; out-of-range values are clamped on save) rather than ignoring the
  // change, which would snap the controlled input back to its old value. `intHandler`
  // rounds for the integer-typed Rust fields (a stray float is rejected by serde);
  // `numHandler` keeps the raw value for the one float field (capture_diff_threshold).
  const intHandler =
    <K extends keyof Settings>(key: K) =>
    (e: ChangeEvent<HTMLInputElement>) => {
      const v = e.currentTarget.valueAsNumber;
      if (Number.isFinite(v)) set(key, Math.round(v) as Settings[K]);
      else if (e.currentTarget.value === "") set(key, 0 as Settings[K]);
    };

  const numHandler =
    <K extends keyof Settings>(key: K) =>
    (e: ChangeEvent<HTMLInputElement>) => {
      const v = e.currentTarget.valueAsNumber;
      if (Number.isFinite(v)) set(key, v as Settings[K]);
      else if (e.currentTarget.value === "") set(key, 0 as Settings[K]);
    };

  // Tier changes hot-apply immediately (persisted + applied to the live provider via
  // set_model_tier); the draft updates optimistically and reverts if the call fails.
  const changeTier = (lane: ModelLane, tier: ModelTier) => {
    const key = lane === "vision" ? "models_vision_tier" : "models_answer_tier";
    const prev = draft[key];
    set(key, tier);
    setTier.mutate(
      { lane, tier },
      {
        onSuccess: () => {
          setBaseline((b) => (b ? { ...b, [key]: tier } : b));
          toast.success(`${lane === "vision" ? "Vision" : "Answer"} model → ${TIER_LABEL[tier]}`);
        },
        onError: (e) => {
          set(key, prev);
          toast.error(String(e));
        },
      },
    );
  };

  const save = () => {
    const clean = sanitizeSettings(draft);
    const clamped = JSON.stringify(clean) !== JSON.stringify(draft);
    setDraft(clean); // reflect any clamped/rounded values back into the form
    setSettings.mutate(clean, {
      onSuccess: () => {
        setBaseline(clean);
        toast.success(
          clamped ? "Settings saved (some values clamped to valid ranges)" : "Settings saved",
        );
      },
      onError: (e) => toast.error(String(e)),
    });
  };

  const reset = () => {
    setDraft(baseline);
    setMonitorsText(baseline.capture_monitors.join(", "));
    setAppsText(baseline.privacy_excluded_apps.join(", "));
  };

  const dirty = JSON.stringify(draft) !== JSON.stringify(baseline);
  const saving = setSettings.isPending;

  // Partial state — surface that models may still be starting: while the readiness
  // probe is in flight, or either lane is still "unknown" (pre-init) / "initializing".
  // Optional chaining guards a partially-populated readiness payload.
  const modelsLoading =
    readiness.isLoading ||
    readiness.data?.embed_model?.status === "initializing" ||
    readiness.data?.embed_model?.status === "unknown" ||
    readiness.data?.sidecar?.status === "initializing" ||
    readiness.data?.sidecar?.status === "unknown";

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4 p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-col">
          <span className="eyebrow">Settings</span>
          <span className="text-body text-ink-muted font-body">
            Everything stays on this device. {dirty ? "You have unsaved changes." : "All changes saved."}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {dirty && <Chip tone="warn">unsaved</Chip>}
          <Button variant="ghost" onClick={reset} disabled={!dirty || saving}>
            Reset
          </Button>
          <Button variant="primary" onClick={save} disabled={!dirty || saving}>
            {saving ? "Saving…" : "Save"}
          </Button>
        </div>
      </div>

      <Panel title="Capture">
        <div className="flex flex-col gap-4">
          <Field
            label="Capture interval (ms)"
            type="number"
            min={250}
            value={draft.capture_interval_ms}
            onChange={intHandler("capture_interval_ms")}
            hint={`How often the screen is sampled. ${APPLY_CAPTURE}`}
          />
          <Field
            label="Change threshold"
            type="number"
            min={0}
            max={1}
            step={0.001}
            value={draft.capture_diff_threshold}
            onChange={numHandler("capture_diff_threshold")}
            hint={`Fraction of the frame that must change to keep it (0–1). ${APPLY_CAPTURE}`}
          />
          <Field
            label="Monitors"
            value={monitorsText}
            onChange={(e) => {
              setMonitorsText(e.currentTarget.value);
              set("capture_monitors", parseIntList(e.currentTarget.value));
            }}
            hint={`Comma-separated monitor indices (0-based). Empty = all monitors. ${APPLY_CAPTURE}`}
          />
        </div>
      </Panel>

      <Panel title="Storage">
        <div className="flex flex-col gap-4">
          <Field
            label="JPEG quality"
            type="number"
            min={1}
            max={100}
            value={draft.storage_jpeg_quality}
            onChange={intHandler("storage_jpeg_quality")}
            hint={`Higher = sharper frames, larger database. ${APPLY_CAPTURE}`}
          />
          <Field
            label="Max width (px)"
            type="number"
            min={320}
            value={draft.storage_max_width}
            onChange={intHandler("storage_max_width")}
            hint={`Captured frames are downscaled to this width. ${APPLY_CAPTURE}`}
          />
          <RetentionControl
            days={draft.storage_retention_days}
            onChange={(d) => set("storage_retention_days", d)}
          />
        </div>
      </Panel>

      <Panel
        title="Models"
        action={modelsLoading ? <Chip tone="warn">models loading…</Chip> : undefined}
      >
        <div className="flex flex-col gap-4">
          <ModelTierPicker
            lane="vision"
            value={draft.models_vision_tier}
            onChange={(t) => changeTier("vision", t)}
            hint={APPLY_NOW}
            disabled={setTier.isPending}
          />
          <ModelTierPicker
            lane="answer"
            value={draft.models_answer_tier}
            onChange={(t) => changeTier("answer", t)}
            hint={APPLY_NOW}
            disabled={setTier.isPending}
          />
          <Toggle
            label="Show model thinking"
            checked={draft.answer_thinking}
            onChange={(v) => set("answer_thinking", v)}
            hint={`Stream the answer model's reasoning before its reply. ${APPLY_ASK}`}
          />
        </div>
      </Panel>

      <Panel title="Enrichment">
        <div className="flex flex-col gap-4">
          <Toggle
            label="Embed OCR text"
            checked={draft.enrich_embed_text}
            onChange={(v) => set("enrich_embed_text", v)}
            hint={`Index recognised text as vectors for semantic search. ${APPLY_RESTART}`}
          />
          <Toggle
            label="Embed images"
            checked={draft.enrich_image_embeddings}
            onChange={(v) => set("enrich_image_embeddings", v)}
            hint={`Also embed frame images (more compute). ${APPLY_RESTART}`}
          />
          <Field
            label="Worker concurrency"
            type="number"
            min={1}
            max={16}
            value={draft.enrich_worker_concurrency}
            onChange={intHandler("enrich_worker_concurrency")}
            hint={`How many enrichment jobs run at once. ${APPLY_RESTART}`}
          />
          <ScheduleControl
            timerEnabled={draft.enrich_vision_timer_enabled}
            timerIntervalMs={draft.enrich_vision_timer_interval_ms}
            idleEnabled={draft.enrich_vision_idle_enabled}
            idleSecs={draft.enrich_vision_idle_secs}
            onChange={patch}
          />
        </div>
      </Panel>

      <Panel title="Privacy">
        <div className="flex flex-col gap-4">
          <Field
            label="Excluded apps"
            value={appsText}
            onChange={(e) => {
              setAppsText(e.currentTarget.value);
              set("privacy_excluded_apps", parseStrList(e.currentTarget.value));
            }}
            hint={`Comma-separated app names; captures are skipped while one is in the foreground. ${APPLY_CAPTURE}`}
          />
          <Toggle
            label="Pause on lock"
            checked={draft.privacy_pause_on_lock}
            onChange={(v) => set("privacy_pause_on_lock", v)}
            hint={`Stop capturing while the workstation is locked. ${APPLY_CAPTURE}`}
          />
        </div>
      </Panel>

      <Panel title="Sidecar (advanced)">
        <div className="flex flex-col gap-4">
          <Field
            label="Idle TTL (seconds)"
            type="number"
            min={0}
            value={draft.sidecar_idle_ttl_secs}
            onChange={intHandler("sidecar_idle_ttl_secs")}
            hint={`Unload the model after this many idle seconds (0 keeps it loaded). ${APPLY_RESTART}`}
          />
          <Field
            label="GPU layers (n-gpu-layers)"
            type="number"
            min={0}
            value={draft.sidecar_ngl}
            onChange={intHandler("sidecar_ngl")}
            hint={`How many model layers to offload to the GPU. ${APPLY_RESTART}`}
          />
        </div>
      </Panel>
    </div>
  );
}
