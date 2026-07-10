import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

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

  it("saves the edited draft", async () => {
    render(
      <WikiPageEditor content="original" onDeleted={() => {}} path="wiki/a.md" projectId="p1" />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Page content"), { target: { value: "updated" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ path: "wiki/a.md", content: "updated" }),
    );
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
