import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "./dialog";

function Harness({ onOpenChange }: { onOpenChange?: (open: boolean) => void }) {
  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>My dialog</DialogTitle>
          <DialogDescription>The body copy</DialogDescription>
        </DialogHeader>
      </DialogContent>
    </Dialog>
  );
}

describe("Dialog", () => {
  it("renders a built-in close button", () => {
    render(<Harness />);
    expect(screen.getByRole("button", { name: /close/i })).toBeInTheDocument();
  });

  it("requests close when the close button is clicked", () => {
    const onOpenChange = vi.fn();
    render(<Harness onOpenChange={onOpenChange} />);
    fireEvent.click(screen.getByRole("button", { name: /close/i }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("requests close when Escape is pressed", () => {
    const onOpenChange = vi.fn();
    render(<Harness onOpenChange={onOpenChange} />);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("wires the accessible name and description to the title and description", () => {
    render(<Harness />);
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAccessibleName("My dialog");
    expect(dialog).toHaveAccessibleDescription("The body copy");
  });
});
