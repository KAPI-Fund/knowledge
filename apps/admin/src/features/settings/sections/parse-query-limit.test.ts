import { describe, expect, it } from "vitest";

import { parseQueryLimit } from "./parse-query-limit";

describe("parseQueryLimit", () => {
  it("keeps a valid in-range integer", () => {
    expect(parseQueryLimit("12", 5)).toBe(12);
    expect(parseQueryLimit("1", 5)).toBe(1);
    expect(parseQueryLimit("100", 5)).toBe(100);
  });

  it("floors fractional values", () => {
    expect(parseQueryLimit("7.9", 5)).toBe(7);
  });

  it("falls back on empty, non-numeric, zero, negative, or over-cap input", () => {
    expect(parseQueryLimit("", 5)).toBe(5);
    expect(parseQueryLimit("abc", 5)).toBe(5);
    expect(parseQueryLimit("0", 5)).toBe(5);
    expect(parseQueryLimit("-3", 5)).toBe(5);
    expect(parseQueryLimit("101", 5)).toBe(5);
  });
});
