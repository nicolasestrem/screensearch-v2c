// openExternal — open a URL in the OS default browser, never the app's own WebView
// (which would unmount the whole UI). Tauri v2 does not open plain `target="_blank"`
// anchors, so every external link — the NavRail version link and the report/answer
// markdown links — routes through the opener plugin (0.3.1 D4, #57). Scoped to http(s)
// by the `opener:allow-open-url` capability.
import { openUrl } from "@tauri-apps/plugin-opener";

export function openExternal(url: string | undefined): void {
  if (!url) return;
  void openUrl(url).catch(() => {
    /* opener unavailable (browser dev) or blocked by scope — nothing safe to fall back to */
  });
}
