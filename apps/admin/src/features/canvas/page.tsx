import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useParams } from "react-router-dom";

import { cn } from "@/lib/utils";

import { extractUrl } from "./api";
import { CanvasBoard } from "./canvas-board";
import { CanvasToolbar } from "./canvas-toolbar";
import { ChatPanel, type SkillNodePayload } from "./chat-panel";
import { HistorySidebar } from "./history-sidebar";
import { useCanvas, useSaveCanvas } from "./queries";
import { runCanvasNode } from "./stream";
import type { CanvasDocument, CanvasNode } from "./types";
import { useAutosave, type SaveStatus } from "./use-autosave";

function emptyDoc(): CanvasDocument {
  return { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } };
}

export function CanvasPage() {
  const { canvasId } = useParams();
  const canvas = useCanvas(canvasId);
  const save = useSaveCanvas(canvasId ?? "");
  const [doc, setDoc] = useState<CanvasDocument | null>(null);
  const [title, setTitle] = useState("");
  const [selectedNodeIds, setSelectedNodeIds] = useState<string[]>([]);
  const loadedId = useRef<string | undefined>(undefined);

  const onSave = useCallback(
    (value: CanvasDocument) => save.mutateAsync({ title, document: value }),
    [save, title],
  );

  const { status, reset } = useAutosave({ value: doc ?? emptyDoc(), delayMs: 800, onSave });

  useEffect(() => {
    // Load the document only when the canvas identity changes so local edits
    // and autosave writes are not clobbered by react-query refetches.
    if (canvas.data && loadedId.current !== canvas.data.id) {
      loadedId.current = canvas.data.id;
      setDoc(canvas.data.document);
      setTitle(canvas.data.title);
      setSelectedNodeIds([]);
      reset(canvas.data.document);
    }
  }, [canvas.data, reset]);

  const patchNodeData = useCallback((nodeId: string, patch: Record<string, unknown>) => {
    setDoc((prev) =>
      prev
        ? {
            ...prev,
            nodes: prev.nodes.map((n) =>
              n.id === nodeId ? { ...n, data: { ...n.data, ...patch } } : n,
            ),
          }
        : prev,
    );
  }, []);

  const runNode = useCallback(
    (nodeId: string) => {
      if (!canvasId) {
        return;
      }
      patchNodeData(nodeId, { status: "running", error: null });
      void runCanvasNode(canvasId, nodeId, {
        onDelta: () => {},
        onDone: (payload) => {
          setDoc((prev) => {
            if (!prev) {
              return prev;
            }
            return {
              ...prev,
              nodes: prev.nodes.map((n) => {
                if (n.id !== nodeId) {
                  return n;
                }
                const versions = Array.isArray(n.data.versions)
                  ? (n.data.versions as unknown[])
                  : [];
                const version =
                  n.type === "ai_image"
                    ? { id: payload.versionId, url: payload.url, createdAt: payload.createdAt }
                    : {
                        id: payload.versionId,
                        content: payload.content,
                        createdAt: payload.createdAt,
                      };
                return {
                  ...n,
                  data: {
                    ...n.data,
                    status: "idle",
                    error: null,
                    versions: [...versions, version],
                    activeVersionId: payload.versionId,
                  },
                };
              }),
            };
          });
        },
        onError: (message) => patchNodeData(nodeId, { status: "error", error: message }),
      });
    },
    [canvasId, patchNodeData],
  );

  const fetchUrlNode = useCallback(
    (nodeId: string) => {
      const node = doc?.nodes.find((n) => n.id === nodeId);
      const url = typeof node?.data.url === "string" ? node.data.url : "";
      if (!url) {
        return;
      }
      patchNodeData(nodeId, { status: "loading", error: null });
      void extractUrl(url)
        .then((result) => {
          if (result.status === "ok") {
            patchNodeData(nodeId, {
              status: "idle",
              error: null,
              title: result.title,
              markdown: result.markdown,
            });
          } else {
            patchNodeData(nodeId, { status: "error", error: result.error ?? "fetch failed" });
          }
        })
        .catch((error: unknown) => {
          patchNodeData(nodeId, {
            status: "error",
            error: error instanceof Error ? error.message : "fetch failed",
          });
        });
    },
    [doc, patchNodeData],
  );

  const addSkillNode = useCallback((payload: SkillNodePayload) => {
    const source = payload.node as { type: CanvasNode["type"]; data?: Record<string, unknown> };
    const id = crypto.randomUUID();
    setDoc((prev) => {
      if (!prev) {
        return prev;
      }
      const position = nextNodePosition(prev);
      const node: CanvasNode = {
        id,
        type: source.type,
        x: position.x,
        y: position.y,
        w: 280,
        h: 160,
        data: source.data ?? {},
      };
      const existingIds = new Set(prev.nodes.map((n) => n.id));
      const sourceIds = Array.isArray(source.data?.sourceNodeIds)
        ? (source.data?.sourceNodeIds as unknown[]).filter(
            (s): s is string => typeof s === "string" && existingIds.has(s),
          )
        : [];
      const newEdges = sourceIds.map((src) => ({
        id: crypto.randomUUID(),
        source: src,
        target: id,
      }));
      return { ...prev, nodes: [...prev.nodes, node], edges: [...prev.edges, ...newEdges] };
    });
  }, []);

  return (
    <div className="flex h-full min-h-0">
      <HistorySidebar activeId={canvasId} />
      <div className="flex min-w-0 flex-1 flex-col">
        <CanvasHeader title={title} status={status} onRetry={() => doc && void onSave(doc)} actions={doc ? <CanvasToolbar onAdd={addSkillNode} /> : null} />
        {doc ? (
          <div className="min-h-0 flex-1">
            <CanvasBoard key={canvasId} document={doc} onChange={setDoc} onRunNode={runNode} onFetchUrl={fetchUrlNode} onSelectionChange={setSelectedNodeIds} />
          </div>
        ) : (
          <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
            从左侧选择或“新建画布”开始。
          </div>
        )}
      </div>
      <ChatPanel canvasId={canvasId ?? ""} selectedNodeIds={selectedNodeIds} onSkillNode={addSkillNode} />
    </div>
  );
}

const statusLabels: Record<SaveStatus, { text: string; className: string }> = {
  idle: { text: "", className: "text-muted-foreground" },
  pending: { text: "待保存…", className: "text-muted-foreground" },
  saving: { text: "保存中…", className: "text-muted-foreground" },
  saved: { text: "已保存", className: "text-emerald-600" },
  error: { text: "保存失败 · 重试", className: "text-destructive" },
};

interface CanvasHeaderProps {
  title: string;
  status: SaveStatus;
  onRetry: () => void;
  actions?: ReactNode;
}

function CanvasHeader({ title, status, onRetry, actions }: CanvasHeaderProps) {
  const label = statusLabels[status];
  return (
    <header className="flex items-center justify-between gap-2 border-b px-4 py-2">
      <span className="truncate text-sm font-semibold">{title || "未命名画布"}</span>
      <div className="flex items-center gap-3">
        {actions}
        {label.text ? (
          <button
            type="button"
            onClick={status === "error" ? onRetry : undefined}
            disabled={status !== "error"}
            className={cn("text-xs", label.className, status !== "error" && "cursor-default")}
          >
            {label.text}
          </button>
        ) : null}
      </div>
    </header>
  );
}

function nextNodePosition(doc: CanvasDocument): { x: number; y: number } {
  const n = doc.nodes.length;
  return { x: 80 + (n % 6) * 48, y: 80 + (n % 6) * 48 };
}
