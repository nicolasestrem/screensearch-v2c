// Render a route's `Component` inside the providers it needs at runtime: a fresh
// QueryClient (retries OFF so a rejected queryFn surfaces the error state on the
// first settle instead of after backoff) and a MemoryRouter (routes use
// useNavigate / useSearchParams). Each call gets its own client so query caches
// never bleed between tests.
import type { ReactElement } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { render } from "@testing-library/react";

export function renderRoute(ui: ReactElement, initialPath = "/") {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0 },
      mutations: { retry: false },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter
        initialEntries={[initialPath]}
        future={{ v7_startTransition: true, v7_relativeSplatPath: true }}
      >
        {ui}
      </MemoryRouter>
    </QueryClientProvider>,
  );
}
