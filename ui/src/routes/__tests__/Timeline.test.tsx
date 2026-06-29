// Timeline view-state contract (UI_REFERENCE §4): the Scanline panel must render
// the right state for each backend outcome. We mock the typed command layer (the
// single `invoke` seam) and drive `get_timeline`; `get_frames` (thumbnails) is
// kept resolved so only the state under test varies.
import { describe, it, expect, beforeEach, vi } from "vitest";
import { screen } from "@testing-library/react";

import { Component as Timeline } from "../Timeline";
import { renderRoute } from "../../test/renderRoute";
import * as cmd from "../../lib/ipc/commands";

vi.mock("../../lib/ipc/commands");

beforeEach(() => {
  vi.mocked(cmd.getFrames).mockResolvedValue([]);
  vi.mocked(cmd.getNearestFrame).mockResolvedValue(null);
});

describe("Timeline view states", () => {
  it("shows a loading skeleton while the timeline query is pending", () => {
    vi.mocked(cmd.getTimeline).mockReturnValue(new Promise(() => {}));

    const { container } = renderRoute(<Timeline />);

    // Pending: a layout-reserving skeleton, never the empty/error copy.
    expect(container.querySelector(".animate-pulse")).not.toBeNull();
    expect(screen.queryByText(/no captures in this range/i)).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows the empty invitation when the range has no captures", async () => {
    vi.mocked(cmd.getTimeline).mockResolvedValue([]);

    renderRoute(<Timeline />);

    expect(await screen.findByText(/no captures in this range/i)).toBeInTheDocument();
  });

  it("shows an error with retry when the timeline query fails", async () => {
    vi.mocked(cmd.getTimeline).mockRejectedValue(new Error("kernel down"));

    renderRoute(<Timeline />);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/couldn't load the timeline/i);
    expect(screen.getByRole("button", { name: /try again/i })).toBeInTheDocument();
  });

  it("renders the scrubbable ribbon when buckets are present", async () => {
    vi.mocked(cmd.getTimeline).mockResolvedValue([{ start: 0, end: 1_000, count: 7 }]);

    renderRoute(<Timeline />);

    // ScanlineTimeline is a real slider (role="slider"); its presence proves the
    // populated branch rendered.
    expect(await screen.findByRole("slider")).toBeInTheDocument();
    expect(screen.queryByText(/no captures in this range/i)).toBeNull();
  });
});
