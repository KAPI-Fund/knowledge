import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ErrorState, ForbiddenState, LoadingState } from "./states";

describe("shared states", () => {
  it("LoadingState renders the requested number of skeletons", () => {
    const { container } = render(<LoadingState rows={3} />);
    expect(container.querySelectorAll('[data-slot="skeleton"]').length).toBe(3);
  });

  it("ErrorState renders title and description", () => {
    render(<ErrorState description="It failed" title="Boom" />);
    expect(screen.getByText("Boom")).toBeInTheDocument();
    expect(screen.getByText("It failed")).toBeInTheDocument();
  });

  it("ForbiddenState renders a default access message", () => {
    render(<ForbiddenState />);
    expect(screen.getByText("Access denied")).toBeInTheDocument();
  });
});
