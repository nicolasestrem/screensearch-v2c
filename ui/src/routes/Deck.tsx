// Deck (/) — at-a-glance home: capture status, today's activity, jump-back-in.
// Foundation scaffold (M1+M2); the live deck body lands in M3. Exported as
// `Component` for React Router's lazy route convention.
import { ScreenScaffold } from "../components/ScreenScaffold";

export function Component() {
  return (
    <ScreenScaffold
      title="Deck"
      purpose="At a glance: capture status, today's activity, and where to jump back in."
    />
  );
}
