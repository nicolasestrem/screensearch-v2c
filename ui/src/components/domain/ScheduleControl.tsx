// ScheduleControl (UI_REFERENCE §5) — the deferred-vision enrichment schedule.
// Vision tagging is never real-time (`03 §5`): on-demand tagging (from a Moment) is
// always available; the timer and idle lanes are independent opt-ins, each with a
// threshold (`Settings`, see specs/06_PATCH_PLAN). The thresholds are surfaced in
// minutes for readability and converted to the stored units (ms / seconds) on edit.
// Both lanes apply on the next app restart, labelled honestly here.
import { Field, Toggle } from "../primitives";

export interface ScheduleControlProps {
  timerEnabled: boolean;
  timerIntervalMs: number;
  idleEnabled: boolean;
  idleSecs: number;
  batchSize: number;
  onChange: (
    patch: Partial<{
      enrich_vision_timer_enabled: boolean;
      enrich_vision_timer_interval_ms: number;
      enrich_vision_idle_enabled: boolean;
      enrich_vision_idle_secs: number;
      enrich_vision_batch_size: number;
    }>,
  ) => void;
}

export function ScheduleControl({
  timerEnabled,
  timerIntervalMs,
  idleEnabled,
  idleSecs,
  batchSize,
  onChange,
}: ScheduleControlProps) {
  // Shown in minutes; a cleared field shows 0 (a transient value to type over) rather
  // than snapping back — the Settings save clamps both thresholds to their valid range.
  const timerMinutes = Math.round(timerIntervalMs / 60_000);
  const idleMinutes = Math.round(idleSecs / 60);

  return (
    <div className="flex flex-col gap-4">
      <p className="text-caption text-ink-faint font-body">
        Vision tagging never runs in real time. On-demand tagging (from a moment) is always
        available — turn on a schedule to also tag untagged frames in the background.
      </p>

      <Toggle
        label="Tag on a timer"
        checked={timerEnabled}
        onChange={(v) => onChange({ enrich_vision_timer_enabled: v })}
        hint="Periodically tag a batch of untagged frames."
      />
      {timerEnabled && (
        <Field
          label="Timer interval (minutes)"
          type="number"
          min={1}
          value={timerMinutes}
          onChange={(e) => {
            const m = e.currentTarget.valueAsNumber;
            if (Number.isFinite(m) && m >= 1) {
              onChange({ enrich_vision_timer_interval_ms: Math.round(m) * 60_000 });
            } else if (e.currentTarget.value === "") {
              onChange({ enrich_vision_timer_interval_ms: 0 });
            }
          }}
          hint="Applies on restart."
        />
      )}

      <Toggle
        label="Tag while idle"
        checked={idleEnabled}
        onChange={(v) => onChange({ enrich_vision_idle_enabled: v })}
        hint="Tag only after you've been away from the keyboard for a while."
      />
      {idleEnabled && (
        <Field
          label="Idle threshold (minutes)"
          type="number"
          min={1}
          value={idleMinutes}
          onChange={(e) => {
            const m = e.currentTarget.valueAsNumber;
            if (Number.isFinite(m) && m >= 1) {
              onChange({ enrich_vision_idle_secs: Math.round(m) * 60 });
            } else if (e.currentTarget.value === "") {
              onChange({ enrich_vision_idle_secs: 0 });
            }
          }}
          hint="Applies on restart."
        />
      )}

      {(timerEnabled || idleEnabled) && (
        <Field
          label="Frames per run"
          type="number"
          min={1}
          max={500}
          value={batchSize}
          onChange={(e) => {
            const n = e.currentTarget.valueAsNumber;
            if (Number.isFinite(n) && n >= 1) {
              onChange({ enrich_vision_batch_size: Math.round(n) });
            } else if (e.currentTarget.value === "") {
              onChange({ enrich_vision_batch_size: 0 });
            }
          }}
          hint="Most untagged frames a scheduled run queues. Already-queued frames are skipped. Applies on the next run."
        />
      )}
    </div>
  );
}
