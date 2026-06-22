// RouteError — the per-route error boundary element (UI_REFERENCE §6: a thrown
// render never blanks the whole app). Shown when a route's loader/render throws.
import { useNavigate, useRouteError } from "react-router-dom";
import { ErrorState } from "../components/primitives";

export function RouteError() {
  const error = useRouteError();
  const navigate = useNavigate();
  const message =
    error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : "Something went wrong rendering this view.";

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
