// Ported from upstream_llm_wiki/src/lib/agent-file-activity.ts L3-49
// (summarizeAgentFileChange): bounded common-prefix/common-suffix hunk diff.

const MAX_DIFF_LINES = 240;
const MAX_DIFF_CHARS = 48_000;

function splitLines(value: string): string[] {
  if (value.length === 0) return [];
  return value.replace(/\r\n/g, "\n").split("\n");
}

export interface FileDiffSummary {
  operation: "created" | "modified";
  additions: number;
  deletions: number;
  diff: string;
}

/**
 * Build a bounded, display-oriented diff. Writes commonly replace a complete
 * file or append one chunk, so a common-prefix/common-suffix hunk is both
 * deterministic and substantially cheaper than an unbounded LCS matrix.
 */
export function diffFileContent(beforeContent: string | null, afterContent: string): FileDiffSummary {
  const before = splitLines(beforeContent ?? "");
  const after = splitLines(afterContent);
  let prefix = 0;
  while (prefix < before.length && prefix < after.length && before[prefix] === after[prefix]) {
    prefix += 1;
  }
  let suffix = 0;
  while (
    suffix < before.length - prefix &&
    suffix < after.length - prefix &&
    before[before.length - 1 - suffix] === after[after.length - 1 - suffix]
  ) {
    suffix += 1;
  }

  const removed = before.slice(prefix, before.length - suffix);
  const added = after.slice(prefix, after.length - suffix);
  const diffLines = [
    `@@ -${prefix + 1},${removed.length} +${prefix + 1},${added.length} @@`,
    ...removed.map((line) => `-${line}`),
    ...added.map((line) => `+${line}`),
  ];
  let diff = diffLines.slice(0, MAX_DIFF_LINES).join("\n");
  if (diffLines.length > MAX_DIFF_LINES) diff += "\n… diff truncated";
  if (diff.length > MAX_DIFF_CHARS) diff = `${diff.slice(0, MAX_DIFF_CHARS)}\n… diff truncated`;

  return {
    operation: beforeContent === null ? "created" : "modified",
    additions: added.length,
    deletions: removed.length,
    diff,
  };
}
