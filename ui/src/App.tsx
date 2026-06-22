// App root — providers + the router. The P2 live-timeline App is replaced by the
// full "Command Deck" shell (UI_REFERENCE). The shell, screens, and live data
// hang off the route table in app/router.tsx.
import { RouterProvider } from "react-router-dom";
import { Providers } from "./app/providers";
import { router } from "./app/router";

export function App() {
  return (
    <Providers>
      <RouterProvider router={router} />
    </Providers>
  );
}
