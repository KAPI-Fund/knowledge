import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { StatusPill } from "./status-pill";

describe("StatusPill", () => {
  it("renders the raw status label", () => {
    render(<StatusPill value="running" />);
    expect(screen.getByText("running")).toBeInTheDocument();
  });

  it("maps a known status to its variant classes", () => {
    render(<StatusPill value="succeeded" />);
    expect(screen.getByText("succeeded").className).toContain("bg-emerald-50");
  });

  it("maps completed onto the succeeded variant", () => {
    render(<StatusPill value="completed" />);
    expect(screen.getByText("completed").className).toContain("bg-emerald-50");
  });

  it("falls back to the queued variant for unknown statuses", () => {
    render(<StatusPill value="mystery" />);
    expect(screen.getByText("mystery").className).toContain("bg-muted");
  });
});
