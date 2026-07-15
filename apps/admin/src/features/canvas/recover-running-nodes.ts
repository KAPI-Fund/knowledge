import type { CanvasDocument } from "./types";

// A node run over SSE (analyze/image/search/url) only reports back to the page
// session that started it. After a reload that session is gone, so a persisted
// "running" status can never resolve — reset it to a retryable error. html
// skill nodes are excluded: their run lives in a server-side job that the
// job poller re-attaches to via data.jobId.
export function recoverOrphanRunningNodes(doc: CanvasDocument): CanvasDocument {
  const orphaned = doc.nodes.some(
    (n) => n.type !== "html" && n.data.status === "running",
  );
  if (!orphaned) {
    return doc;
  }
  return {
    ...doc,
    nodes: doc.nodes.map((n) =>
      n.type !== "html" && n.data.status === "running"
        ? {
            ...n,
            data: { ...n.data, status: "error", error: "运行已中断（页面被刷新或关闭），请重试" },
          }
        : n,
    ),
  };
}
