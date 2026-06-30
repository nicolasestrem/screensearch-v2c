// DEV-ONLY deterministic route-state override (KNOWN_GAPS #43). A pure module —
// no React, no side effects — so the override logic is unit-reasonable and so the
// whole feature const-folds out of production builds (every caller guards on
// `import.meta.env.DEV`, and Vite's `define` replaces it with `false`, letting the
// tree-shaker drop this module entirely; verified by the absence of `__devState`
// in the emitted bundle).
//
// It forces the query-RESULT FLAGS only — it never fabricates a payload. So the
// real empty/partial/populated outcomes still flow from live data; only the two
// states that are awkward to reproduce on demand (a stuck `loading`, a thrown
// `error`) are synthesised. This honors the gap's "no mocks in production paths"
// constraint: nothing here ships, and even in dev no fake data is invented.
import type { QueryKey, UseQueryResult } from "@tanstack/react-query";

/** The two result states the override can force. The other three (empty / partial
 *  / populated) are reached through real data, never through this override. */
export type DevState = "loading" | "error";

/** URL query param that drives the override, e.g. `?__devState=error`. */
export const DEV_STATE_PARAM = "__devState";
/** Optional param scoping the override to a single query family by its key head,
 *  e.g. `?__devState=loading&__devStateKey=timeline` only stalls `useTimeline`. */
export const DEV_STATE_KEY_PARAM = "__devStateKey";

export interface DevStateConfig {
  /** The forced state, or `null` when no (valid) `__devState` is present. */
  state: DevState | null;
  /** When set, only queries whose key head equals this are overridden. */
  key: string | null;
}

function isDevState(value: string | null): value is DevState {
  return value === "loading" || value === "error";
}

/**
 * Parse the override out of a location search string (e.g. `useSearchParams`'s
 * serialized value, or `window.location.search`). Returns an inert config when
 * the param is absent or unrecognized, so callers can apply it unconditionally.
 */
export function readDevState(search: string): DevStateConfig {
  const params = new URLSearchParams(search);
  const raw = params.get(DEV_STATE_PARAM);
  return {
    state: isDevState(raw) ? raw : null,
    key: params.get(DEV_STATE_KEY_PARAM),
  };
}

/** The stable head of a query key (its first segment), used for `__devStateKey`
 *  scoping. Mirrors the convention in queryKeys.ts where the head names the family. */
function queryKeyHead(queryKey: QueryKey): string | null {
  const head = Array.isArray(queryKey) ? queryKey[0] : queryKey;
  return typeof head === "string" ? head : null;
}

interface ApplyArgs {
  config: DevStateConfig;
  queryKey: QueryKey;
}

/**
 * Return the real result untouched, or — when an override is active and in scope —
 * a shallow clone with the discriminating flags forced to the requested state.
 *
 * The clone always preserves the live `refetch` (so a view's retry button still
 * drives the real query) and the rest of the base fields from `result`, overriding
 * only the discriminators. The override never invents a payload: `data` is forced
 * to `undefined` for both states, exactly as a real first-load / load-error result.
 *
 * Typed against TanStack Query v5's discriminated `UseQueryResult` union: each
 * branch is asserted to the precise variant interface (`...LoadingResult` /
 * `...LoadingErrorResult`), which is a member of that union, so the return stays
 * assignable to `UseQueryResult<TData, TError>` without `any`.
 */
export function applyDevState<TData, TError = Error>(
  result: UseQueryResult<TData, TError>,
  { config, queryKey }: ApplyArgs,
): UseQueryResult<TData, TError> {
  const { state, key } = config;
  if (state === null) return result;
  if (key !== null && key !== queryKeyHead(queryKey)) return result;

  if (state === "loading") {
    return {
      ...result,
      data: undefined,
      error: null,
      isPending: true,
      isLoading: true,
      isFetching: true,
      isError: false,
      isLoadingError: false,
      isRefetchError: false,
      isSuccess: false,
      isPlaceholderData: false,
      status: "pending",
      fetchStatus: "fetching",
    } as UseQueryResult<TData, TError>;
  }

  // state === "error": a first-load error. `refetch` is preserved above via the
  // spread, so the view's retry still calls the live query.
  return {
    ...result,
    data: undefined,
    error: new Error("dev: forced route error") as unknown as TError,
    isError: true,
    isLoadingError: true,
    isRefetchError: false,
    isPending: false,
    isLoading: false,
    isFetching: false,
    isSuccess: false,
    isPlaceholderData: false,
    status: "error",
    fetchStatus: "idle",
  } as UseQueryResult<TData, TError>;
}
