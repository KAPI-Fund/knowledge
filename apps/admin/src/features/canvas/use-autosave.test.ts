import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAutosave } from "./use-autosave";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("useAutosave", () => {
  it("does not save the initial value", () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    renderHook(() => useAutosave({ value: { a: 1 }, delayMs: 100, onSave }));
    act(() => vi.advanceTimersByTime(200));
    expect(onSave).not.toHaveBeenCalled();
  });

  it("saves after the value changes and debounce elapses", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { rerender } = renderHook(({ value }) => useAutosave({ value, delayMs: 100, onSave }), {
      initialProps: { value: { a: 1 } },
    });
    rerender({ value: { a: 2 } });
    await act(async () => {
      vi.advanceTimersByTime(100);
    });
    expect(onSave).toHaveBeenCalledWith({ a: 2 });
  });

  it("does not save when reset seeds a new baseline equal to the next value", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { result, rerender } = renderHook(
      ({ value }) => useAutosave({ value, delayMs: 100, onSave }),
      { initialProps: { value: { a: 1 } } },
    );
    // Simulate a load: reset baseline to the loaded value, then rerender with it.
    act(() => result.current.reset({ a: 9 }));
    rerender({ value: { a: 9 } });
    await act(async () => {
      vi.advanceTimersByTime(200);
    });
    expect(onSave).not.toHaveBeenCalled();
  });

  it("does not save an unchanged value (same JSON)", () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { rerender } = renderHook(({ value }) => useAutosave({ value, delayMs: 100, onSave }), {
      initialProps: { value: { a: 1 } },
    });
    rerender({ value: { a: 1 } });
    act(() => vi.advanceTimersByTime(200));
    expect(onSave).not.toHaveBeenCalled();
  });

  it("returns status to idle when reset after a save", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { result, rerender } = renderHook(
      ({ value }) => useAutosave({ value, delayMs: 100, onSave }),
      { initialProps: { value: { a: 1 } } },
    );
    rerender({ value: { a: 2 } });
    await act(async () => {
      vi.advanceTimersByTime(100);
    });
    expect(result.current.status).toBe("saved");
    act(() => result.current.reset({ a: 3 }));
    expect(result.current.status).toBe("idle");
  });
});
