// RetentionControl (UI_REFERENCE §5) — the screenshot-retention window. A backend sweep
// runs at startup and hourly; past the window it removes the screenshot but KEEPS the
// recognized text (still searchable; the Moment view shows a text reconstruction). 0
// keeps screenshots forever.
import { Field } from "../primitives";

export interface RetentionControlProps {
  days: number;
  onChange: (days: number) => void;
}

export function RetentionControl({ days, onChange }: RetentionControlProps) {
  return (
    <Field
      label="Keep screenshots (days)"
      type="number"
      min={0}
      value={days}
      onChange={(e) => {
        const d = e.currentTarget.valueAsNumber;
        if (Number.isFinite(d) && d >= 0) onChange(Math.round(d));
        // A cleared field is NaN — fall back to 0 so the input doesn't snap back.
        else if (e.currentTarget.value === "") onChange(0);
      }}
      hint="After this many days a capture's screenshot is removed to save space, but its recognized text is kept and stays searchable. 0 keeps screenshots forever."
    />
  );
}
