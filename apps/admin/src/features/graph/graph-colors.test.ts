import { describe, expect, it } from "vitest";
import { hexToRgba, mixColor, nodeColor, nodeSize } from "./graph-colors";

describe("graph-colors", () => {
  it("maps known node types to palette colors", () => {
    expect(nodeColor("concept")).toBe("#c084fc");
    expect(nodeColor("entity")).toBe("#60a5fa");
  });

  it("falls back to a hashed custom color for unknown types", () => {
    expect(nodeColor("madeuptype")).toMatch(/^#[0-9a-f]{6}$/i);
  });

  it("converts hex to rgba", () => {
    expect(hexToRgba("#60a5fa", 0.5)).toBe("rgba(96,165,250,0.5)");
  });

  it("mixes two colors by ratio", () => {
    expect(mixColor("#000000", "#ffffff", 0.5)).toBe("#808080");
  });

  it("returns the base size when there are no links", () => {
    expect(nodeSize(0, 0, 10, 1)).toBe(8);
  });
});
