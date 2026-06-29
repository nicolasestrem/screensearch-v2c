// Recall view-state contract (UI_REFERENCE §4), search mode. The search query is
// idle until something is typed, so a freshly mounted Recall must show the search
// invitation (never a blank or zero-result dead end) and offer all three modes.
// Deeper search states (loading/no-match/results) ride the keystroke debounce +
// virtualizer and are exercised via the SearchBody states in the route itself.
import { describe, it, expect, beforeEach, vi } from "vitest";
import { screen } from "@testing-library/react";

import { Component as Recall } from "../Recall";
import { renderRoute } from "../../test/renderRoute";
import * as cmd from "../../lib/ipc/commands";
import type { Readiness } from "../../bindings/Readiness";
import type { Settings } from "../../bindings/Settings";

vi.mock("../../lib/ipc/commands");

beforeEach(() => {
  vi.resetAllMocks();
  // Minimal resolved reads so the readiness/settings queries don't error; the
  // search-invite state does not depend on their contents.
  vi.mocked(cmd.getReadiness).mockResolvedValue({} as unknown as Readiness);
  vi.mocked(cmd.getSettings).mockResolvedValue({} as unknown as Settings);
});

describe("Recall search mode", () => {
  it("shows the search invitation before anything is typed", async () => {
    renderRoute(<Recall />);

    expect(await screen.findByText(/search your screen history/i)).toBeInTheDocument();
    // search() must not be called for an empty query.
    expect(cmd.search).not.toHaveBeenCalled();
  });

  it("offers all three recall modes", () => {
    renderRoute(<Recall />);

    // The mode switcher is a tablist (role="tab"), distinct from the submit button.
    expect(screen.getByRole("tab", { name: /search/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /ask/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /reports/i })).toBeInTheDocument();
  });
});
