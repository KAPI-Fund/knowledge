import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { CompositionInput, CompositionTextarea } from "./composition-input";

describe("CompositionInput", () => {
  it("reflects the external value when idle", () => {
    const { rerender } = render(<CompositionInput value="a" onValueChange={vi.fn()} />);
    const input = screen.getByRole("textbox") as HTMLInputElement;
    expect(input.value).toBe("a");

    rerender(<CompositionInput value="b" onValueChange={vi.fn()} />);
    expect(input.value).toBe("b");
  });

  it("propagates plain (non-IME) typing immediately", () => {
    const onValueChange = vi.fn();
    render(<CompositionInput value="" onValueChange={onValueChange} />);
    const input = screen.getByRole("textbox");

    fireEvent.change(input, { target: { value: "hello" } });

    expect(onValueChange).toHaveBeenCalledWith("hello");
  });

  it("does not propagate mid-composition and commits exactly once on compositionEnd", () => {
    const onValueChange = vi.fn();
    render(<CompositionInput value="" onValueChange={onValueChange} />);
    const input = screen.getByRole("textbox");

    fireEvent.compositionStart(input);
    fireEvent.change(input, { target: { value: "ni" } });
    fireEvent.change(input, { target: { value: "nihao" } });
    expect(onValueChange).not.toHaveBeenCalled();

    fireEvent.compositionEnd(input, { target: { value: "你好" } });
    expect(onValueChange).toHaveBeenCalledTimes(1);
    expect(onValueChange).toHaveBeenLastCalledWith("你好");
  });

  it("keeps the in-progress composition when a stale external value arrives (the duplication guard)", () => {
    const onValueChange = vi.fn();
    const { rerender } = render(<CompositionInput value="" onValueChange={onValueChange} />);
    const input = screen.getByRole("textbox") as HTMLInputElement;

    fireEvent.compositionStart(input);
    fireEvent.change(input, { target: { value: "ni" } });

    // An async document round-trip pushes a stale value back mid-composition.
    // It must NOT clobber the composition buffer, or the IME duplicates characters.
    rerender(<CompositionInput value="" onValueChange={onValueChange} />);

    expect(input.value).toBe("ni");
  });
});

describe("CompositionTextarea", () => {
  it("does not propagate mid-composition and commits once on compositionEnd", () => {
    const onValueChange = vi.fn();
    render(<CompositionTextarea value="" onValueChange={onValueChange} />);
    const textarea = screen.getByRole("textbox");

    fireEvent.compositionStart(textarea);
    fireEvent.change(textarea, { target: { value: "shi" } });
    expect(onValueChange).not.toHaveBeenCalled();

    fireEvent.compositionEnd(textarea, { target: { value: "是" } });
    expect(onValueChange).toHaveBeenCalledTimes(1);
    expect(onValueChange).toHaveBeenLastCalledWith("是");
  });

  it("keeps the in-progress composition when a stale external value arrives", () => {
    const onValueChange = vi.fn();
    const { rerender } = render(<CompositionTextarea value="" onValueChange={onValueChange} />);
    const textarea = screen.getByRole("textbox") as HTMLTextAreaElement;

    fireEvent.compositionStart(textarea);
    fireEvent.change(textarea, { target: { value: "shi" } });
    rerender(<CompositionTextarea value="" onValueChange={onValueChange} />);

    expect(textarea.value).toBe("shi");
  });
});
