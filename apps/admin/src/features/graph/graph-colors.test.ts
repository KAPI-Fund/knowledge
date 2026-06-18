import { describe, expect, it } from "vitest";
import {
  GRAPH_PALETTE,
  hexToRgba,
  mixColor,
  nodeColor,
  NODE_TYPE_COLORS,
  nodeSize,
  SIGMA_BASE_SETTINGS,
} from "./graph-colors";

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

describe("SIGMA_BASE_SETTINGS", () => {
  it("carries the light-palette edge, node, and label colors", () => {
    expect(SIGMA_BASE_SETTINGS.defaultEdgeColor).toBe(GRAPH_PALETTE.defaultEdge);
    expect(SIGMA_BASE_SETTINGS.defaultNodeColor).toBe(NODE_TYPE_COLORS.other);
    expect(SIGMA_BASE_SETTINGS.labelColor).toEqual({ color: GRAPH_PALETTE.label });
  });

  it("preserves the existing render defaults", () => {
    expect(SIGMA_BASE_SETTINGS.defaultNodeType).toBe("circle");
    expect(SIGMA_BASE_SETTINGS.hideEdgesOnMove).toBe(true);
    expect(SIGMA_BASE_SETTINGS.hideLabelsOnMove).toBe(true);
    expect(SIGMA_BASE_SETTINGS.renderEdgeLabels).toBe(false);
    expect(SIGMA_BASE_SETTINGS.stagePadding).toBe(30);
  });
});
