import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";

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
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        overlay: resolve(__dirname, "overlay.html"),
      },
      output: {
        // Split heavy, rarely-changing vendor code into its own chunks so the
        // app shell stays small and cacheable (UI_REFERENCE §8). react-markdown
        // is only imported by /recall, so route-splitting already keeps it out
        // of the initial chunk; this just isolates the big shared libs.
        manualChunks(id) {
          const normalized = id.replace(/\\/g, "/");
          if (!normalized.includes("/node_modules/")) return undefined;
          if (
            normalized.includes("/react/") ||
            normalized.includes("/react-dom/") ||
            normalized.includes("/scheduler/")
          ) {
            return "react-vendor";
          }
          if (
            normalized.includes("/react-router-dom/") ||
            normalized.includes("/@remix-run/router/")
          ) {
            return "router";
          }
          if (normalized.includes("/@tanstack/react-query/")) {
            return "query";
          }
          return undefined;
        },
      },
    },
  },
});
