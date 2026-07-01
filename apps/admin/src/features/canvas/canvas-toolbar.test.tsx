import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./kb-picker", () => ({ KbProjectPicker: () => <div>kb-picker</div> }));

import { CanvasToolbar } from "./canvas-toolbar";

describe("CanvasToolbar", () => {
  it("adds a note node", () => {
    const onAdd = vi.fn();
    render(<CanvasToolbar onAdd={onAdd} />);
    fireEvent.click(screen.getByRole("button", { name: "笔记" }));
    expect(onAdd).toHaveBeenCalledWith({ node: { type: "note", data: {} }, x: 0, y: 0 });
  });

  it("opens the kb picker dialog", () => {
    render(<CanvasToolbar onAdd={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "知识库" }));
    expect(screen.getByText("kb-picker")).toBeInTheDocument();
  });
});
