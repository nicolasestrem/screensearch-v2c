import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri 2 + Vite. The dev server port is pinned to match `devUrl` in
// ../src-tauri/tauri.conf.json. Build target is a modern Chromium baseline
// because the app is Windows-only (WebView2 / Edge Chromium).
// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Tauri shows its own startup output; don't let Vite clear it.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: "chrome110",
  },
});
