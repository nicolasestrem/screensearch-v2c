// RetentionControl (UI_REFERENCE §5) — the capture-retention window. The value is
// persisted honestly but **not yet enforced**: no purge job exists, so nothing is
// auto-deleted today (a retention sweep is a planned follow-up, logged in
// specs/07_KNOWN_GAPS.md). The hint says so plainly rather than implying deletion.
import { Field } from "../primitives";

export interface RetentionControlProps {
  days: number;
  onChange: (days: number) => void;
}

export function RetentionControl({ days, onChange }: RetentionControlProps) {
  return (
    <Field
      label="Retention (days)"
      type="number"
      min={0}
      value={days}
      onChange={(e) => {
        const d = e.currentTarget.valueAsNumber;
        if (Number.isFinite(d) && d >= 0) onChange(Math.round(d));
      }}
      hint="0 keeps captures forever. Recorded but not yet enforced — no automatic deletion runs today (a purge job is a planned follow-up)."
    />
  );
}
