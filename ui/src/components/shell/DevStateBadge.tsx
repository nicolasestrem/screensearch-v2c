// DEV-ONLY badge surfacing the active route-state override (KNOWN_GAPS #43). A
// fixed-corner pill reading e.g. "DEV STATE: error" so it's obvious during an
// audit that the view is being forced rather than reflecting live data. Mounted in
// AppShell behind `import.meta.env.DEV`, so it (and this file) tree-shake out of
// production builds. Renders nothing when no `?__devState=…` is active.
import { useSearchParams } from "react-router-dom";

import { Chip } from "../primitives";
import { readDevState } from "../../lib/dev/devState";

export function DevStateBadge() {
  const [searchParams] = useSearchParams();
  const { state, key } = readDevState(searchParams.toString());
  if (state === null) return null;

  return (
    <div className="fixed bottom-4 left-4 z-toast" aria-hidden="true">
      <Chip tone="warn" dot>
        DEV STATE: {state}
        {key ? ` · ${key}` : ""}
      </Chip>
    </div>
  );
}
