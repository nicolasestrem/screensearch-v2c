// Insights view-state contract (UI_REFERENCE §4): loading → error → empty →
// populated, driven entirely by the `get_insights` aggregate. Charts are
// token-styled divs (no canvas), so the populated assertion is stable in jsdom.
import { describe, it, expect, beforeEach, vi } from "vitest";
import { screen } from "@testing-library/react";

import { Component as Insights } from "../Insights";
import { renderRoute } from "../../test/renderRoute";
import * as cmd from "../../lib/ipc/commands";
import type { InsightsSummary } from "../../bindings/InsightsSummary";

vi.mock("../../lib/ipc/commands");

const populated: InsightsSummary = {
  total_frames: 12,
  tagged_frames: 12,
  captures: [{ start: 0, end: 1_000, count: 12 }],
  top_apps: [{ app: "Code", count: 9 }],
  activity_breakdown: [{ activity: "coding", count: 9 }],
};

beforeEach(() => {
  vi.resetAllMocks();
});

describe("Insights view states", () => {
  it("shows a loading skeleton while the aggregate query is pending", () => {
    vi.mocked(cmd.getInsights).mockReturnValue(new Promise(() => {}));

    const { container } = renderRoute(<Insights />);

    expect(container.querySelector(".animate-pulse")).not.toBeNull();
    expect(screen.queryByText(/not enough history yet/i)).toBeNull();
  });

  it("shows an error with retry when the aggregate query fails", async () => {
    vi.mocked(cmd.getInsights).mockRejectedValue(new Error("aggregate failed"));

    renderRoute(<Insights />);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/couldn't compute insights/i);
    expect(screen.getByRole("button", { name: /try again/i })).toBeInTheDocument();
  });

  it("shows the honest empty state when no frames were captured", async () => {
    vi.mocked(cmd.getInsights).mockResolvedValue({
      total_frames: 0,
      tagged_frames: 0,
      captures: [],
      top_apps: [],
      activity_breakdown: [],
    });

    renderRoute(<Insights />);

    expect(await screen.findByText(/not enough history yet/i)).toBeInTheDocument();
  });

  it("renders the charts when there is history", async () => {
    vi.mocked(cmd.getInsights).mockResolvedValue(populated);

    renderRoute(<Insights />);

    expect(await screen.findByText("12 captures")).toBeInTheDocument();
    expect(screen.getByText("Captures over time")).toBeInTheDocument();
    expect(screen.queryByText(/not enough history yet/i)).toBeNull();
  });
});
