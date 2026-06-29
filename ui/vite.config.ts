import { defineConfig } from "vitest/config";
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
    rollupOptions: {
      output: {
        // Split heavy, rarely-changing vendor code into its own chunks so the
        // app shell stays small and cacheable (UI_REFERENCE §8). react-markdown
        // is only imported by /recall, so route-splitting already keeps it out
        // of the initial chunk; this just isolates the big shared libs.
        manualChunks: {
          "react-vendor": ["react", "react-dom", "react-router-dom"],
          query: ["@tanstack/react-query"],
        },
      },
    },
  },
  // Vitest: jsdom DOM, a shared setup (jest-dom matchers + a ResizeObserver
  // polyfill jsdom lacks), and explicit imports (globals off) so test files
  // stay honest about what they use.
  test: {
    environment: "jsdom",
    globals: false,
    setupFiles: ["./src/test/setup.ts"],
    css: false,
  },
});
