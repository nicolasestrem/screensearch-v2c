// Resolve a stored frame's relative `image_path` to a URL the WebView can load.
//
// Frames are written under $APPDATA/frames/** and recorded in the DB as a path
// relative to the app-data dir (e.g. "frames/day-19500/1684...-0.jpg", see
// kernel::capture_loop::image_paths). We join that onto the (memoized) app-data
// dir and hand it to Tauri's asset protocol via convertFileSrc — which is scoped
// to "$APPDATA/frames/**" and allowed by the CSP (img-src asset:) in
// tauri.conf.json. Outside the Tauri shell (npm run dev) appDataDir/convertFileSrc
// throw; callers get `null` and render a placeholder (the no-Tauri dev path).
import { convertFileSrc } from "@tauri-apps/api/core";

/** Memoized app-data dir (resolved once); `null` when not in the Tauri shell. */
let rootPromise: Promise<string | null> | null = null;

function appDataRoot(): Promise<string | null> {
  if (!rootPromise) {
    rootPromise = (async () => {
      try {
        const { appDataDir } = await import("@tauri-apps/api/path");
        const dir = await appDataDir();
        return dir.replace(/[\\/]+$/, ""); // strip any trailing separator
      } catch {
        return null; // dev mode / no Tauri runtime
      }
    })();
  }
  return rootPromise;
}

/** Memoized resolved URLs, keyed by the DB-relative image path. */
const urlCache = new Map<string, string | null>();

/**
 * Resolve a relative `image_path` to an asset URL, or `null` if unavailable
 * (no path, or no Tauri runtime). Results are memoized per path.
 */
export async function frameSrc(imagePath: string | null | undefined): Promise<string | null> {
  if (!imagePath) return null;
  const cached = urlCache.get(imagePath);
  if (cached !== undefined) return cached;

  const root = await appDataRoot();
  let url: string | null = null;
  if (root !== null) {
    try {
      url = convertFileSrc(`${root}/${imagePath}`);
    } catch {
      url = null;
    }
  }
  urlCache.set(imagePath, url);
  return url;
}
