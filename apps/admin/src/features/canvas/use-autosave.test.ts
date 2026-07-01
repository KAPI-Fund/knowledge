import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAutosave } from "./use-autosave";

describe("useAutosave", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("fires a single save after rapid edits settle", async () => {
    const save = vi.fn(() => Promise.resolve());
    const { result, rerender } = renderHook(
      ({ doc }) => useAutosave({ value: doc, delayMs: 500, onSave: save }),
      { initialProps: { doc: { v: 0 } } },
    );

    for (let v = 1; v <= 5; v++) {
      rerender({ doc: { v } });
      act(() => {
        vi.advanceTimersByTime(100);
      });
    }
    expect(save).not.toHaveBeenCalled();
    await act(async () => {
      vi.advanceTimersByTime(500);
    });
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith({ v: 5 });
    expect(result.current.status).toBe("saved");
  });

  it("reports save-failed on rejection", async () => {
    const save = vi.fn(() => Promise.reject(new Error("nope")));
    const { result, rerender } = renderHook(
      ({ doc }) => useAutosave({ value: doc, delayMs: 300, onSave: save }),
      { initialProps: { doc: { v: 0 } } },
    );
    rerender({ doc: { v: 1 } });
    await act(async () => {
      vi.advanceTimersByTime(300);
    });
    expect(result.current.status).toBe("error");
  });
});
