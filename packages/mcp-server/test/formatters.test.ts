import { describe, expect, it } from "vitest";

import {
  formatFileTree,
  formatGraph,
  formatReviews,
  formatSearchResults,
  truncateText,
} from "../src/formatters.js";

describe("formatFileTree", () => {
  it("returns a placeholder when empty", () => {
    expect(formatFileTree([], false)).toBe("No files found.");
  });

  it("walks nested directories with indentation", () => {
    const tree = [
      {
        name: "wiki",
        path: "wiki",
        isDir: true,
        children: [{ name: "index.md", path: "wiki/index.md", isDir: false }],
      },
    ];
    const out = formatFileTree(tree, false);
    expect(out).toContain("📁 wiki");
    expect(out).toContain("  📄 wiki/index.md");
  });

  it("prepends a truncation warning when the API truncated", () => {
    const out = formatFileTree([{ name: "a.md", path: "a.md", isDir: false }], true);
    expect(out).toMatch(/\[warning\] File tree was truncated/);
  });
});

describe("formatSearchResults", () => {
  it("returns a no-results placeholder", () => {
    expect(formatSearchResults("foo", { results: [] })).toBe('No results for "foo".');
  });

  it("renders meta and entries", () => {
    const out = formatSearchResults("q", {
      results: [{ path: "wiki/a.md", title: "A", snippet: "hit", score: 0.5, vectorScore: 0.9 }],
      mode: "hybrid",
      tokenHits: 2,
      vectorHits: 1,
    });
    expect(out).toContain('# Search results for "q"');
    expect(out).toContain("Mode: hybrid");
    expect(out).toContain("## 1. A");
    expect(out).toContain("Snippet: hit");
  });
});

describe("formatReviews", () => {
  it("returns a placeholder when empty", () => {
    expect(formatReviews({ reviews: [] })).toBe("No review items found.");
  });

  it("renders review entries with options", () => {
    const out = formatReviews({
      reviews: [
        {
          id: "r1",
          type: "missing-page",
          title: "Missing Topic",
          description: "We should add this.",
          status: "unresolved",
          options: [{ label: "Confirm", action: "confirm" }],
        },
      ],
    });
    expect(out).toContain("## 1. Missing Topic");
    expect(out).toContain("Type: missing-page");
    expect(out).toContain("Options: Confirm (confirm)");
  });
});

describe("formatGraph", () => {
  it("counts node types and lists top nodes", () => {
    const out = formatGraph(
      [
        { id: "a", label: "A", type: "concept", linkCount: 3 },
        { id: "b", label: "B", type: "entity", linkCount: 1 },
      ],
      [{ source: "a", target: "b" }],
    );
    expect(out).toContain("Nodes: 2");
    expect(out).toContain("Edges: 1");
    expect(out).toContain("- concept: 1");
    expect(out).toContain("A (concept, 3 links)");
  });
});

describe("truncateText", () => {
  it("returns input unchanged under the limit", () => {
    expect(truncateText("short", 100)).toBe("short");
  });

  it("appends a truncation note when too large", () => {
    const long = "x".repeat(200);
    const out = truncateText(long, 50);
    expect(out.startsWith("x".repeat(50))).toBe(true);
    expect(out).toMatch(/\[truncated: \d+ bytes omitted\]/);
  });
});
