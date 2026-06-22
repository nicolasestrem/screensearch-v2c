// Settings (/settings) — capture, models (tiers), enrichment schedule, privacy,
// retention. Foundation scaffold (M1+M2); the controls and persist/apply flow
// land in M5.
import { ScreenScaffold } from "../components/ScreenScaffold";

export function Component() {
  return (
    <ScreenScaffold
      title="Settings"
      purpose="Tune capture, model tiers, the enrichment schedule, privacy, and retention."
    />
  );
}
