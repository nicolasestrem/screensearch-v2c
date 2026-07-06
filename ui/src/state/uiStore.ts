// Ephemeral UI state only (UI_REFERENCE §6: Zustand is for transient view state,
// never server data — that lives in TanStack Query). Holds command-palette
// visibility, the selected timeline range, the focused frame, and the Settings
// Advanced-tier expander state.
import { create } from "zustand";
import type { TimeRange } from "../bindings/TimeRange";

interface UiState {
  /** ⌘K command palette open/closed. */
  paletteOpen: boolean;
  openPalette: () => void;
  closePalette: () => void;
  togglePalette: () => void;

  /** The time window the recall views operate over (null = default window). */
  selectedRange: TimeRange | null;
  setSelectedRange: (range: TimeRange | null) => void;

  /** The frame currently focused on the timeline (drives hover/scrub preview). */
  focusedFrameId: number | null;
  setFocusedFrameId: (id: number | null) => void;

  /**
   * Which Settings Advanced groups are expanded, by group id (0.3.2 PR5, D6).
   * Absent = collapsed — every group starts collapsed each session; living here (not
   * component state) the choice survives navigating away and back within the session,
   * with no new persistence machinery (UI_REFERENCE §3).
   */
  settingsExpanded: Record<string, boolean>;
  toggleSettingsGroup: (id: string) => void;
}

export const useUiStore = create<UiState>((set) => ({
  paletteOpen: false,
  openPalette: () => set({ paletteOpen: true }),
  closePalette: () => set({ paletteOpen: false }),
  togglePalette: () => set((s) => ({ paletteOpen: !s.paletteOpen })),

  selectedRange: null,
  setSelectedRange: (selectedRange) => set({ selectedRange }),

  focusedFrameId: null,
  setFocusedFrameId: (focusedFrameId) => set({ focusedFrameId }),

  settingsExpanded: {},
  toggleSettingsGroup: (id) =>
    set((s) => ({
      settingsExpanded: { ...s.settingsExpanded, [id]: !s.settingsExpanded[id] },
    })),
}));
