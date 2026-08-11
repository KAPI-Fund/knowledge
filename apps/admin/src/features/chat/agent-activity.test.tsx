import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { AgentActivity } from "./agent-activity";
import type { AgentEvent } from "./agent-types";

describe("AgentActivity", () => {
  it("renders nothing when there are no displayable events", () => {
    const events: AgentEvent[] = [
      { type: "agentStart", sessionId: "s1" },
      { type: "turnStart", mode: "standard" },
      { type: "done", sessionId: "s1" },
    ];
    const { container } = render(<AgentActivity events={events} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("pairs toolStart with toolEnd and expands input/output detail", async () => {
    const user = userEvent.setup();
    const events: AgentEvent[] = [
      { type: "toolStart", tool: "wiki.search", input: "graph layouts" },
      { type: "toolEnd", tool: "wiki.search", output: "3 results" },
    ];
    render(<AgentActivity events={events} />);

    expect(screen.getByText("wiki.search")).toBeInTheDocument();
    const toggle = screen.getByRole("button", { expanded: false });
    await user.click(toggle);
    expect(screen.getByText("3 results")).toBeInTheDocument();
  });

  it("renders file changes, references, and errors", () => {
    const events: AgentEvent[] = [
      { type: "fileChanged", path: "wiki/overview.md", tool: "wiki.write_page", existedBefore: true },
      {
        type: "referenceAdded",
        reference: { title: "Overview", path: "wiki/overview.md", kind: "wiki" },
      },
      { type: "error", message: "budget exhausted" },
    ];
    render(<AgentActivity events={events} />);

    expect(screen.getByText("wiki/overview.md")).toBeInTheDocument();
    expect(screen.getByText("modified")).toBeInTheDocument();
    expect(screen.getByText("Overview")).toBeInTheDocument();
    expect(screen.getByText("budget exhausted")).toBeInTheDocument();
  });

  it("marks created files distinctly from modified ones", () => {
    const events: AgentEvent[] = [
      { type: "fileChanged", path: "notes/new.md", tool: "workspace.write_file", existedBefore: false },
    ];
    render(<AgentActivity events={events} />);
    expect(screen.getByText("created")).toBeInTheDocument();
  });
});
