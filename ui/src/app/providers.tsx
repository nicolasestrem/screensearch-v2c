// App-wide providers. TanStack Query owns all server-state (UI_REFERENCE §6).
// Defaults: don't refetch on window focus (a desktop app is always "focused"),
// retry once (a transient IPC hiccup), short stale time so live-event patches and
// manual refetches stay cheap.
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
      staleTime: 5_000,
    },
  },
});

export function Providers({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}
