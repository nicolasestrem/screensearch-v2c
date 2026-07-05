// useAppVersion — the running app's version string (from Tauri's app metadata), or
// null when there is no Tauri runtime (browser dev via `npm run dev`), so callers hide
// version UI outside the packaged app (the same "no Tauri in the browser" guard used in
// AppShell). `getVersion()` reads the value baked into `tauri.conf.json` at build time
// and is covered by `core:default` — no capability edit (0.3.1 D4, #57-partial).
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

export function useAppVersion(): string | null {
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    getVersion()
      .then((v) => {
        if (active) setVersion(v);
      })
      .catch(() => {
        /* no Tauri runtime in browser dev — leave null so version UI stays hidden */
      });
    return () => {
      active = false;
    };
  }, []);
  return version;
}
