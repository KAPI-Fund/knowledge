import { afterEach, describe, expect, it, vi } from "vitest";

import { uuid } from "./uuid";

const V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

describe("uuid", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("uses crypto.randomUUID when available", () => {
    const randomUUID = vi.fn(() => "11111111-1111-4111-8111-111111111111");
    vi.stubGlobal("crypto", { randomUUID, getRandomValues: vi.fn() });
    expect(uuid()).toBe("11111111-1111-4111-8111-111111111111");
    expect(randomUUID).toHaveBeenCalledOnce();
  });

  it("builds a v4 uuid from getRandomValues when randomUUID is missing (insecure context)", () => {
    // Insecure origins expose getRandomValues but not randomUUID.
    vi.stubGlobal("crypto", {
      getRandomValues: (bytes: Uint8Array) => {
        for (let i = 0; i < bytes.length; i += 1) {
          bytes[i] = i;
        }
        return bytes;
      },
    });
    const value = uuid();
    expect(value).toMatch(V4);
  });

  it("falls back to Math.random when Web Crypto is absent entirely", () => {
    vi.stubGlobal("crypto", undefined);
    expect(uuid()).toMatch(V4);
  });

  it("returns distinct values across calls", () => {
    vi.unstubAllGlobals();
    expect(uuid()).not.toBe(uuid());
  });
});
