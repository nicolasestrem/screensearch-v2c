// ScrollContainerContext — a ref to the AppShell `<main>` element, the single scroll
// context for every route (UI_REFERENCE §8, D9). Routes that virtualize a long list
// (Recall) read this so the virtualizer scrolls the shell content pane itself instead
// of introducing a nested scroller — one scroll context per route, the scrollbar at the
// pane edge. Provided once by AppShell; `null` until `<main>` mounts (it mounts before
// route effects run, so consumers see a live ref).
import { createContext, useContext, type RefObject } from "react";

export type ShellScrollRef = RefObject<HTMLElement | null>;

const ScrollContainerContext = createContext<ShellScrollRef | null>(null);

export const ScrollContainerProvider = ScrollContainerContext.Provider;

/** The shell `<main>` scroll container ref, or `null` outside an AppShell (e.g. the
 *  separate overlay window). Callers guard on `.current` being present. */
export function useShellScrollContainer(): ShellScrollRef | null {
  return useContext(ScrollContainerContext);
}
