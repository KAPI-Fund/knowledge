import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { KbItem } from "./kb-grouping";

const useHook = vi.fn();
vi.mock("./use-accessible-kbs", () => ({
  useAccessibleKnowledgeBases: () => useHook(),
}));

import { KbProjectPicker } from "./kb-picker";

function item(overrides: Partial<KbItem> & Pick<KbItem, "id" | "name">): KbItem {
  return {
    role: "viewer",
    groupKey: "personal",
    groupLabel: "Personal",
    groupOrder: 0,
    ...overrides,
  };
}

function loaded(items: KbItem[]) {
  useHook.mockReturnValue({ items, isLoading: false, isError: false });
}

describe("KbProjectPicker", () => {
  it("shows skeletons while loading", () => {
    useHook.mockReturnValue({ items: [], isLoading: true, isError: false });
    const { container } = render(<KbProjectPicker onPick={vi.fn()} />);
    expect(container.querySelectorAll('[data-slot="skeleton"]').length).toBeGreaterThan(0);
  });

  it("shows the empty hint when there are no accessible knowledge bases", () => {
    loaded([]);
    render(<KbProjectPicker onPick={vi.fn()} />);
    expect(screen.getByText(/no knowledge bases/i)).toBeInTheDocument();
  });

  it("renders grouped knowledge bases with their access role", () => {
    loaded([
      item({ id: "p", name: "My Notes", role: "owner" }),
      item({ id: "o", name: "Acme KB", role: "editor", groupKey: "org:1", groupLabel: "Acme", groupOrder: 1 }),
    ]);
    render(<KbProjectPicker onPick={vi.fn()} />);
    expect(screen.getByText("Personal")).toBeInTheDocument();
    expect(screen.getByText("Acme")).toBeInTheDocument();
    expect(screen.getByText("My Notes")).toBeInTheDocument();
    expect(screen.getByText("Owner")).toBeInTheDocument();
    expect(screen.getByText("Editor")).toBeInTheDocument();
  });

  it("calls onPick with the id and name when a row is clicked", () => {
    const onPick = vi.fn();
    loaded([item({ id: "kb1", name: "My Notes" })]);
    render(<KbProjectPicker onPick={onPick} />);
    fireEvent.click(screen.getByRole("button", { name: /my notes/i }));
    expect(onPick).toHaveBeenCalledWith({ id: "kb1", name: "My Notes" });
  });

  it("filters the list as the search query changes", () => {
    loaded([
      item({ id: "a", name: "Marketing KB" }),
      item({ id: "b", name: "Engineering KB" }),
    ]);
    render(<KbProjectPicker onPick={vi.fn()} />);
    fireEvent.change(screen.getByLabelText(/search knowledge bases/i), {
      target: { value: "eng" },
    });
    expect(screen.queryByText("Marketing KB")).not.toBeInTheDocument();
    expect(screen.getByText("Engineering KB")).toBeInTheDocument();
  });
});
