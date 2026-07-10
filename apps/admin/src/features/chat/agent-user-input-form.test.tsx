import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { AgentUserInputRequest } from "./agent-types";
import { AgentUserInputForm } from "./agent-user-input-form";

const request: AgentUserInputRequest = {
  requestId: "req-1",
  title: "Confirm the plan",
  description: "The agent needs a few answers before continuing.",
  fields: [
    {
      id: "approach",
      type: "single",
      label: "Approach",
      options: [
        { label: "Rewrite", value: "rewrite", recommended: true },
        { label: "Patch", value: "patch" },
      ],
    },
    {
      id: "scopes",
      type: "multi",
      label: "Scopes",
      options: [
        { label: "Wiki", value: "wiki" },
        { label: "Sources", value: "sources" },
      ],
    },
    { id: "title", type: "text", label: "Title", defaultValue: "Draft" },
    { id: "notes", type: "textarea", label: "Notes" },
    { id: "proceed", type: "confirm", label: "Proceed?" },
  ],
};

describe("AgentUserInputForm", () => {
  it("renders all field types with title and description", () => {
    render(<AgentUserInputForm onSubmit={vi.fn()} request={request} />);

    expect(screen.getByText("Confirm the plan")).toBeInTheDocument();
    expect(screen.getByText(/needs a few answers/)).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /Rewrite/ })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: /Wiki/ })).toBeInTheDocument();
    expect(screen.getByLabelText("Title")).toHaveValue("Draft");
    expect(screen.getByLabelText("Notes")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "确认" })).toBeInTheDocument();
  });

  it("collects values for every field type on submit", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<AgentUserInputForm onSubmit={onSubmit} request={request} />);

    await user.click(screen.getByRole("radio", { name: /Patch/ }));
    await user.click(screen.getByRole("checkbox", { name: /Wiki/ }));
    await user.click(screen.getByRole("checkbox", { name: /Sources/ }));
    await user.clear(screen.getByLabelText("Title"));
    await user.type(screen.getByLabelText("Title"), "Final title");
    await user.type(screen.getByLabelText("Notes"), "some notes");
    await user.click(screen.getByRole("radio", { name: "确认" }));
    await user.click(screen.getByRole("button", { name: "提交" }));

    expect(onSubmit).toHaveBeenCalledWith({
      approach: "patch",
      scopes: ["wiki", "sources"],
      title: "Final title",
      notes: "some notes",
      proceed: true,
    });
  });

  it("does not submit while disabled", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<AgentUserInputForm disabled onSubmit={onSubmit} request={request} />);

    await user.click(screen.getByRole("button", { name: "提交" }));
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
