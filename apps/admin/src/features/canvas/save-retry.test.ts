import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { retrySaveInBackground } from "./save-retry";

describe("retrySaveInBackground", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("calls the save once when it succeeds", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    retrySaveInBackground(save);
    await vi.runAllTimersAsync();
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("retries with backoff and logs once after exhausting attempts", async () => {
    const err = vi.spyOn(console, "error").mockImplementation(() => {});
    const save = vi.fn().mockRejectedValue(new Error("nope"));
    retrySaveInBackground(save, { attempts: 3, baseDelayMs: 500 });
    await vi.runAllTimersAsync();
    expect(save).toHaveBeenCalledTimes(3);
    expect(err).toHaveBeenCalledTimes(1);
    err.mockRestore();
  });

  it("stops retrying once a save succeeds", async () => {
    const save = vi
      .fn()
      .mockRejectedValueOnce(new Error("nope"))
      .mockResolvedValue(undefined);
    retrySaveInBackground(save, { attempts: 3, baseDelayMs: 500 });
    await vi.runAllTimersAsync();
    expect(save).toHaveBeenCalledTimes(2);
  });
});
