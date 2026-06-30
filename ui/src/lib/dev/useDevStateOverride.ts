// DEV-ONLY helper wrapping a read query's result with the route-state override
// (KNOWN_GAPS #43). Applied at the single `useQuery` seam in queries.ts so every
// view inherits forced loading/error states without per-view wiring.
//
// Production stripping is real because this helper calls NO React hooks: Vite's
// `define` replaces `import.meta.env.DEV` with the literal `false`, so the guard
// becomes the first statement and everything below it (the only references to
// devState.ts) is dead code Rollup folds away — the helper reduces to
// `return result`, a plain identity. We deliberately read `window.location.search`
// instead of `useSearchParams()`: a hook call would NOT be tree-shaken (the helper
// is invoked from the production `queries.ts` call-site), so it would subscribe all
// 17 query consumers to router-history changes in release builds and would require
// every query hook to sit under a <Router> (see PR #60 review). With no hook here,
// the early return raises no Rules-of-Hooks concern. For the dev audit flow the
// param is read on each route render, which is all the documented procedure needs.
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
  if (!import.meta.env.DEV) return result;

  const search = typeof window !== "undefined" ? window.location.search : "";
  const config = readDevState(search);
  return applyDevState(result, { config, queryKey });
}
