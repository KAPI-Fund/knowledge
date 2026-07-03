import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const navigate = vi.fn();
const deleteMutate = vi.fn();

vi.mock("react-router-dom", () => ({ useNavigate: () => navigate }));
vi.mock("./queries", () => ({
  useCanvasList: () => ({ isLoading: false, data: [{ id: "c1", title: "First" }] }),
  useCreateCanvas: () => ({ mutate: vi.fn(), isPending: false }),
  useDeleteCanvas: () => ({ mutate: deleteMutate, isPending: false }),
}));

import { HistorySidebar } from "./history-sidebar";

describe("HistorySidebar collapse", () => {
  it("lists canvases when expanded", () => {
    render(<HistorySidebar activeId="c1" collapsed={false} onToggle={() => {}} />);
    expect(screen.getByText("First")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Collapse canvas list" })).toBeInTheDocument();
  });

  it("keeps the rail visible but hides the list content when collapsed", () => {
    render(<HistorySidebar activeId="c1" collapsed onToggle={() => {}} />);
    // The list slides away; only the expand + new-canvas rail remains.
    expect(screen.queryByText("First")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Expand canvas list" })).toBeInTheDocument();
  });

  it("toggles from the collapse button", () => {
    const onToggle = vi.fn();
    render(<HistorySidebar activeId="c1" collapsed={false} onToggle={onToggle} />);
    fireEvent.click(screen.getByRole("button", { name: "Collapse canvas list" }));
    expect(onToggle).toHaveBeenCalledTimes(1);
  });
});

describe("HistorySidebar delete", () => {
  it("confirms before deleting a canvas", () => {
    deleteMutate.mockClear();
    render(<HistorySidebar activeId="c1" collapsed={false} onToggle={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Delete First" }));
    // A confirmation dialog appears; nothing is deleted yet.
    expect(deleteMutate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(deleteMutate).toHaveBeenCalledTimes(1);
    expect(deleteMutate.mock.calls[0][0]).toBe("c1");
  });

  it("cancels deletion without calling the mutation", () => {
    deleteMutate.mockClear();
    render(<HistorySidebar activeId="c1" collapsed={false} onToggle={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Delete First" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(deleteMutate).not.toHaveBeenCalled();
  });
});
