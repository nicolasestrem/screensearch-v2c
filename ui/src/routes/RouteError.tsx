// RouteError — the per-route error boundary element (UI_REFERENCE §6: a thrown
// render never blanks the whole app). Shown when a route's loader/render throws.
import { isRouteErrorResponse, useNavigate, useRouteError } from "react-router-dom";
import { ErrorState } from "../components/primitives";

/**
 * Best human-readable message for whatever `useRouteError` surfaced: a thrown
 * `Response` (React Router's `ErrorResponse` — `{ status, statusText, data }`,
 * e.g. a 404 or a loader Response), a real `Error`, a string, or any object
 * carrying a `message` — falling back to a generic line.
 */
function routeErrorMessage(error: unknown): string {
  if (isRouteErrorResponse(error)) {
    const detail = typeof error.data === "string" && error.data ? error.data : error.statusText;
    return detail ? `${error.status} — ${detail}` : `${error.status}`;
  }
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    const m = (error as { message: unknown }).message;
    if (typeof m === "string" && m) return m;
  }
  return "Something went wrong rendering this view.";
}

export function RouteError() {
  const error = useRouteError();
  const navigate = useNavigate();
  const message = routeErrorMessage(error);

  return (
    <div className="p-6">
      <ErrorState
        title="This view hit an error"
        message={message}
        onRetry={() => navigate(0)}
        retryLabel="Reload"
      />
    </div>
  );
}
