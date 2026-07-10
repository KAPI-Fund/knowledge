import { describe, expect, it } from "vitest";

import { diffFileContent } from "./file-diff";

describe("diffFileContent", () => {
  it("marks new files as created with all lines added", () => {
    const result = diffFileContent(null, "a\nb");
    expect(result.operation).toBe("created");
    expect(result.additions).toBe(2);
    expect(result.deletions).toBe(0);
    expect(result.diff).toBe("@@ -1,0 +1,2 @@\n+a\n+b");
  });

  it("isolates the changed hunk between common prefix and suffix", () => {
    const result = diffFileContent("keep\nold\ntail", "keep\nnew\ntail");
    expect(result.operation).toBe("modified");
    expect(result.additions).toBe(1);
    expect(result.deletions).toBe(1);
    expect(result.diff).toBe("@@ -2,1 +2,1 @@\n-old\n+new");
  });

  it("reports a no-op change as an empty hunk", () => {
    const result = diffFileContent("same", "same");
    expect(result.additions).toBe(0);
    expect(result.deletions).toBe(0);
  });

  it("normalizes CRLF before diffing", () => {
    const result = diffFileContent("a\r\nb", "a\nb");
    expect(result.additions).toBe(0);
    expect(result.deletions).toBe(0);
  });

  it("truncates diffs past the line bound", () => {
    const before = null;
    const after = Array.from({ length: 400 }, (_, index) => `line-${index}`).join("\n");
    const result = diffFileContent(before, after);
    expect(result.additions).toBe(400);
    expect(result.diff.endsWith("… diff truncated")).toBe(true);
    // Header + 239 payload lines survive the 240-line cap.
    expect(result.diff.split("\n").length).toBe(241);
  });
});
