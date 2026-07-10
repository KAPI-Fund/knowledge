import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  saveMutateAsync: vi.fn(),
  deleteMutateAsync: vi.fn(),
}));

vi.mock("./queries", () => ({
  useSaveFileContentMutation: () => ({ mutateAsync: mocks.saveMutateAsync, isPending: false }),
  useDeleteWikiPagesMutation: () => ({ mutateAsync: mocks.deleteMutateAsync, isPending: false }),
}));

import { WikiPageEditor, isEditableWikiPath } from "./wiki-page-editor";

describe("WikiPageEditor", () => {
  beforeEach(() => {
    mocks.saveMutateAsync.mockReset().mockResolvedValue({ path: "wiki/a.md", created: false });
    mocks.deleteMutateAsync
      .mockReset()
      .mockResolvedValue({ deletedPaths: ["wiki/a.md"], rewrittenFiles: 2 });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("only treats wiki markdown paths as editable", () => {
    expect(isEditableWikiPath("wiki/concepts/a.md")).toBe(true);
    expect(isEditableWikiPath("raw/sources/a.md")).toBe(false);
    expect(isEditableWikiPath("wiki/media/a.png")).toBe(false);
  });

  it("renders nothing for non-wiki paths", () => {
    const { container } = render(
      <WikiPageEditor content="x" onDeleted={() => {}} path="raw/sources/a.md" projectId="p1" />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("auto-saves the draft after the 1s debounce", async () => {
    vi.useFakeTimers();
    render(
      <WikiPageEditor content="original" onDeleted={() => {}} path="wiki/a.md" projectId="p1" />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Page content"), { target: { value: "updated" } });
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1000);
    expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ path: "wiki/a.md", content: "updated" });
  });

  it("does not save when the draft matches the loaded content", async () => {
    vi.useFakeTimers();
    render(
      <WikiPageEditor content="original" onDeleted={() => {}} path="wiki/a.md" projectId="p1" />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Page content"), { target: { value: "changed" } });
    fireEvent.change(screen.getByLabelText("Page content"), { target: { value: "original" } });
    await vi.advanceTimersByTimeAsync(2000);
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
  });

  it("saves immediately on Ctrl+S", async () => {
    render(
      <WikiPageEditor content="original" onDeleted={() => {}} path="wiki/a.md" projectId="p1" />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const textarea = screen.getByLabelText("Page content");
    fireEvent.change(textarea, { target: { value: "updated" } });
    fireEvent.keyDown(textarea, { ctrlKey: true, key: "s" });
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ path: "wiki/a.md", content: "updated" }),
    );
  });

  it("flushes the pending draft and exits editing on Done", async () => {
    render(
      <WikiPageEditor content="original" onDeleted={() => {}} path="wiki/a.md" projectId="p1" />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Page content"), { target: { value: "updated" } });
    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ path: "wiki/a.md", content: "updated" }),
    );
    await waitFor(() => expect(screen.queryByLabelText("Page content")).not.toBeInTheDocument());
  });

  it("deletes after confirmation and reports the cascade result", async () => {
    const onDeleted = vi.fn();
    render(<WikiPageEditor content="x" onDeleted={onDeleted} path="wiki/a.md" projectId="p1" />);
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    fireEvent.click(await screen.findByRole("button", { name: "Delete page" }));
    await waitFor(() =>
      expect(mocks.deleteMutateAsync).toHaveBeenCalledWith({ paths: ["wiki/a.md"] }),
    );
    expect(onDeleted).toHaveBeenCalled();
    expect(screen.getByText(/rewrote 2 file/i)).toBeInTheDocument();
  });
});
