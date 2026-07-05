// openExternal — open a URL in the OS default browser, never the app's own WebView
// (which would unmount the whole UI). Tauri v2 does not open plain `target="_blank"`
// anchors, so every external link — the NavRail version link and the report/answer
// markdown links — routes through the opener plugin (0.3.1 D4, #57). Scoped to http(s)
// by the `opener:allow-open-url` capability.
import type { MouseEvent } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

export function openExternal(url: string | undefined): void {
  if (!url) return;
  void openUrl(url).catch(() => {
    /* opener unavailable (browser dev) or blocked by scope — nothing safe to fall back to */
  });
}

/** Whether `openExternal` can actually route this URL: an http(s) link inside the packaged
 *  app. Everything else — browser dev (no Tauri runtime), or a non-http(s) scheme the
 *  `opener:allow-open-url` capability would block (`mailto:`, in-page anchors) — must be
 *  left to the anchor's own navigation instead of being swallowed. */
function canOpenExternally(url: string | undefined): url is string {
  return (
    !!url &&
    isTauri() &&
    (url.startsWith("http://") || url.startsWith("https://"))
  );
}

/** onClick for a model-output markdown link (report/answer text): in the packaged app an
 *  http(s) link is intercepted and handed to the OS browser (Tauri v2 ignores plain
 *  `target="_blank"`); every other case falls through to the anchor's own `target="_blank"`
 *  so browser-dev links and non-http(s) schemes still work instead of dying silently. */
export function handleExternalLinkClick(
  e: MouseEvent<HTMLAnchorElement>,
  href: string | undefined,
): void {
  if (!canOpenExternally(href)) return;
  e.preventDefault();
  openExternal(href);
}
