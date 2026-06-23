// RetentionControl (UI_REFERENCE §5) — the capture-retention window. A backend
// sweep runs at startup and hourly; 0 keeps captures forever.
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
        // A cleared field is NaN — fall back to 0 so the input doesn't snap back.
        else if (e.currentTarget.value === "") onChange(0);
      }}
      hint="0 keeps captures forever. Old captures are purged at startup and hourly."
    />
  );
}
