// DEV-ONLY hook wrapping a read query's result with the route-state override
// (KNOWN_GAPS #43). Applied at the single `useQuery` seam in queries.ts so every
// view inherits forced loading/error states without per-view wiring.
//
// Rules-of-Hooks: `useSearchParams` is called UNCONDITIONALLY as the first hook,
// before the `import.meta.env.DEV` early return. In production Vite replaces
// `import.meta.env.DEV` with `false`, so the body below the guard const-folds away
// and the whole helper (plus devState.ts) is dropped by the tree-shaker.
import { useSearchParams } from "react-router-dom";
import type { QueryKey, UseQueryResult } from "@tanstack/react-query";

import { applyDevState, readDevState } from "./devState";

/**
 * Pass a read hook's result through the dev override. A no-op in production (and
 * whenever no `?__devState=…` is present, or it is scoped to another family via
 * `__devStateKey`). See devState.ts for the forcing rules.
 */
export function useMaybeOverride<TData, TError = Error>(
  result: UseQueryResult<TData, TError>,
  queryKey: QueryKey,
): UseQueryResult<TData, TError> {
  // Hook first, unconditionally — keeps Rules-of-Hooks satisfied even though the
  // line below returns early in production.
  const [searchParams] = useSearchParams();
  if (!import.meta.env.DEV) return result;

  const config = readDevState(searchParams.toString());
  return applyDevState(result, { config, queryKey });
}
