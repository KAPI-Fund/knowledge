import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./kb-picker", () => ({ KbProjectPicker: () => <div>kb-picker</div> }));

import { CanvasToolbar } from "./canvas-toolbar";

describe("CanvasToolbar", () => {
  it("adds a note node", () => {
    const onAdd = vi.fn();
    render(<CanvasToolbar onAdd={onAdd} />);
    fireEvent.click(screen.getByRole("button", { name: "Note" }));
    expect(onAdd).toHaveBeenCalledWith({ node: { type: "note", data: {} }, x: 0, y: 0 });
  });

  it("opens the kb picker dialog", () => {
    render(<CanvasToolbar onAdd={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Knowledge base" }));
    expect(screen.getByText("kb-picker")).toBeInTheDocument();
  });

  it("adds a search node", () => {
    const onAdd = vi.fn();
    render(<CanvasToolbar onAdd={onAdd} />);
    fireEvent.click(screen.getByRole("button", { name: "Search" }));
    expect(onAdd).toHaveBeenCalledWith({ node: { type: "search", data: {} }, x: 0, y: 0 });
  });

  it("adds an image node", () => {
    const onAdd = vi.fn();
    render(<CanvasToolbar onAdd={onAdd} />);
    fireEvent.click(screen.getByRole("button", { name: "Image" }));
    expect(onAdd).toHaveBeenCalledWith({ node: { type: "ai_image", data: {} }, x: 0, y: 0 });
  });

  it("adds an analyze node", () => {
    const onAdd = vi.fn();
    render(<CanvasToolbar onAdd={onAdd} />);
    fireEvent.click(screen.getByRole("button", { name: "Analyze" }));
    expect(onAdd).toHaveBeenCalledWith({ node: { type: "ai_analyze", data: {} }, x: 0, y: 0 });
  });
});
