import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { PageHeader } from "./page-header";

describe("PageHeader", () => {
  it("renders the title as a level-1 heading", () => {
    render(<PageHeader title="Tasks" />);
    expect(screen.getByRole("heading", { level: 1, name: "Tasks" })).toBeInTheDocument();
  });

  it("renders the optional description and actions", () => {
    render(
      <PageHeader
        actions={<button type="button">New task</button>}
        description="Background jobs"
        title="Tasks"
      />,
    );
    expect(screen.getByText("Background jobs")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New task" })).toBeInTheDocument();
  });
});
