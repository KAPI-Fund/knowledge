import { useCallback, useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";

import { cn } from "@/lib/utils";

import { CanvasBoard } from "./canvas-board";
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
  const loadedId = useRef<string | undefined>(undefined);

  useEffect(() => {
    // Load the document only when the canvas identity changes so local edits
    // and autosave writes are not clobbered by react-query refetches.
    if (canvas.data && loadedId.current !== canvas.data.id) {
      loadedId.current = canvas.data.id;
      setDoc(canvas.data.document);
      setTitle(canvas.data.title);
    }
  }, [canvas.data]);

  const onSave = useCallback(
    (value: CanvasDocument) => save.mutateAsync({ title, document: value }),
    [save, title],
  );

  const { status } = useAutosave({ value: doc ?? emptyDoc(), delayMs: 800, onSave });

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

  const addSkillNode = useCallback((payload: SkillNodePayload) => {
    const source = payload.node as { type: CanvasNode["type"]; data?: Record<string, unknown> };
    const node: CanvasNode = {
      id: crypto.randomUUID(),
      type: source.type,
      x: payload.x,
      y: payload.y,
      w: 280,
      h: 160,
      data: source.data ?? {},
    };
    setDoc((prev) => (prev ? { ...prev, nodes: [...prev.nodes, node] } : prev));
  }, []);

  return (
    <div className="flex h-full min-h-0">
      <HistorySidebar activeId={canvasId} />
      <div className="flex min-w-0 flex-1 flex-col">
        <CanvasHeader title={title} status={status} onRetry={() => doc && void onSave(doc)} />
        {doc ? (
          <div className="min-h-0 flex-1">
            <CanvasBoard document={doc} onChange={setDoc} onRunNode={runNode} />
          </div>
        ) : (
          <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
            从左侧选择或“新建画布”开始。
          </div>
        )}
      </div>
      <ChatPanel canvasId={canvasId ?? ""} selectedNodeIds={[]} onSkillNode={addSkillNode} />
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
}

function CanvasHeader({ title, status, onRetry }: CanvasHeaderProps) {
  const label = statusLabels[status];
  return (
    <header className="flex items-center justify-between gap-2 border-b px-4 py-2">
      <span className="truncate text-sm font-semibold">{title || "未命名画布"}</span>
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
    </header>
  );
}
