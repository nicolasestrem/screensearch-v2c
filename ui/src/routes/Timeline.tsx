// Timeline (/timeline) — the signature Scanline Timeline browser. Foundation
// scaffold (M1+M2); the canvas density ribbon and scrubbing land in M4.
import { ScreenScaffold } from "../components/ScreenScaffold";

export function Component() {
  return (
    <ScreenScaffold
      title="Timeline"
      purpose="Scrub a continuous filmstrip of captures — density shows when things happened."
    />
  );
}
