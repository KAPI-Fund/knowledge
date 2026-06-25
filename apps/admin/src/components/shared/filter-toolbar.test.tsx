import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { FilterToolbar } from "./filter-toolbar";

describe("FilterToolbar", () => {
  it("renders the search input with placeholder", () => {
    render(
      <FilterToolbar onSearchChange={() => {}} searchPlaceholder="Filter tasks..." searchValue="" />,
    );
    expect(screen.getByPlaceholderText("Filter tasks...")).toBeInTheDocument();
  });

  it("calls onSearchChange when typing", async () => {
    const user = userEvent.setup();
    const onSearchChange = vi.fn();
    render(<FilterToolbar onSearchChange={onSearchChange} searchValue="" />);
    await user.type(screen.getByRole("textbox"), "a");
    expect(onSearchChange).toHaveBeenCalledWith("a");
  });

  it("renders children alongside the search box", () => {
    render(
      <FilterToolbar onSearchChange={() => {}} searchValue="">
        <button type="button">Status</button>
      </FilterToolbar>,
    );
    expect(screen.getByRole("button", { name: "Status" })).toBeInTheDocument();
  });
});
