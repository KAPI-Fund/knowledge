import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  historyQuery: vi.fn(),
  entryQuery: vi.fn(),
  restoreMutate: vi.fn(),
}));

vi.mock("./queries", () => ({
  useFileHistoryQuery: (...args: unknown[]) => mocks.historyQuery(...args),
  useFileHistoryEntryQuery: (...args: unknown[]) => mocks.entryQuery(...args),
  useRestoreFileHistoryMutation: () => ({ mutate: mocks.restoreMutate, isPending: false }),
}));

import { FileHistoryPanel, isRestorableHistoryPath } from "./file-history-panel";

const entry = {
  id: "v1",
  path: "wiki/a.md",
  author: "user-1",
  tool: "editor.save",
  createdAt: "2026-07-10T08:00:00Z",
};

describe("FileHistoryPanel", () => {
  beforeEach(() => {
    mocks.historyQuery.mockReset().mockReturnValue({ data: [entry], isLoading: false, error: null });
    mocks.entryQuery
      .mockReset()
      .mockReturnValue({ data: { ...entry, content: "old" }, isLoading: false, error: null });
    mocks.restoreMutate.mockReset();
  });

  it("classifies restorable paths like the server", () => {
    expect(isRestorableHistoryPath("wiki/a.md")).toBe(true);
    expect(isRestorableHistoryPath("agent-workspace/report/out.svg")).toBe(true);
    expect(isRestorableHistoryPath("wiki/media/image.png")).toBe(false);
    expect(isRestorableHistoryPath("raw/sources/book.md")).toBe(false);
  });

  it("lists entries and shows the diff for the selected version", () => {
    render(<FileHistoryPanel currentContent="new" path="wiki/a.md" projectId="p1" />);
    fireEvent.click(screen.getByRole("button", { name: /history/i }));
    // Query stays disabled until the sheet opens.
    expect(mocks.historyQuery).toHaveBeenLastCalledWith("p1", "wiki/a.md", true);
    fireEvent.click(screen.getByRole("button", { name: /user-1/i }));
    expect(screen.getByText(/-old/)).toBeInTheDocument();
    expect(screen.getByText(/\+new/)).toBeInTheDocument();
  });

  it("restores the selected version after confirmation", async () => {
    render(<FileHistoryPanel currentContent="new" path="wiki/a.md" projectId="p1" />);
    fireEvent.click(screen.getByRole("button", { name: /history/i }));
    fireEvent.click(screen.getByRole("button", { name: /user-1/i }));
    fireEvent.click(screen.getByRole("button", { name: /restore this version/i }));
    fireEvent.click(await screen.findByRole("button", { name: "Restore" }));
    expect(mocks.restoreMutate).toHaveBeenCalledWith(
      { entryId: "v1", path: "wiki/a.md" },
      expect.anything(),
    );
  });

  it("disables restore for non-restorable paths", () => {
    mocks.historyQuery.mockReturnValue({
      data: [{ ...entry, path: "raw/sources/book.md" }],
      isLoading: false,
      error: null,
    });
    render(<FileHistoryPanel currentContent="new" path="raw/sources/book.md" projectId="p1" />);
    fireEvent.click(screen.getByRole("button", { name: /history/i }));
    fireEvent.click(screen.getByRole("button", { name: /user-1/i }));
    expect(screen.getByRole("button", { name: /restore this version/i })).toBeDisabled();
  });
});
