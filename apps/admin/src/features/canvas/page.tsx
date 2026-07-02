import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useParams } from "react-router-dom";

import { cn } from "@/lib/utils";

import { extractUrl, saveCanvas } from "./api";
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

  const { status, reset, flush } = useAutosave({ value: doc ?? emptyDoc(), delayMs: 800, onSave });

  // Flush a pending edit to the currently loaded canvas's own id. `onSave` is
  // bound to the route id, which is wrong once the route has moved on, so we
  // save through the loaded id captured here instead.
  const flushLoaded = useCallback(() => {
    const staleId = loadedId.current;
    if (!staleId) {
      return;
    }
    return flush((value) => saveCanvas(staleId, { title, document: value }));
  }, [flush, title]);

  // A same-instance route change runs the load effect's clear branch; leaving
  // the canvas section entirely unmounts this component and only runs effect
  // cleanups. Flush on unmount too so the last in-debounce edit is not lost.
  // The ref keeps the [] cleanup pinned to the latest loaded id/title/doc.
  const flushLoadedRef = useRef(flushLoaded);
  flushLoadedRef.current = flushLoaded;
  useEffect(() => () => void flushLoadedRef.current(), []);

  useEffect(() => {
    // Only treat data as loaded when it matches the current route id. This
    // component instance is reused across /canvas, /canvas/c1, /canvas/c2, so
    // without this guard the previous board would linger while the new id is
    // still loading (or failed to load) -- and since `save` is already bound
    // to the new id, an edit in that window would autosave the stale document
    // under the wrong canvas.
    const loaded = canvasId && canvas.data?.id === canvasId ? canvas.data : null;
    if (loaded) {
      // Load only when the identity changes so local edits and autosave writes
      // are not clobbered by react-query refetches of the same canvas.
      if (loadedId.current !== loaded.id) {
        loadedId.current = loaded.id;
        setDoc(loaded.document);
        setTitle(loaded.title);
        setSelectedNodeIds([]);
        reset(loaded.document);
      }
      return;
    }
    // No canvas is loaded for the current route: index route, still loading,
    // load error, or stale data from a previous id. Drop any previous board
    // and reset autosave so a pending write cannot land under the new id.
    if (loadedId.current !== undefined) {
      // Persist any edit still inside the autosave debounce before dropping the
      // board. reset() below cancels the timer, and onSave is bound to the new
      // route id, so without flushing to the leaving canvas's own id the last
      // edit made within the debounce window would be lost.
      flushLoaded();
      loadedId.current = undefined;
      setDoc(null);
      setTitle("");
      setSelectedNodeIds([]);
      reset(emptyDoc());
    }
  }, [canvasId, canvas.data, reset, flushLoaded]);

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

  // Persist the current doc to the active canvas before an SSE action. The
  // run/chat endpoints re-read the canvas from the DB, so an edit still inside
  // the autosave debounce would be invisible (node not found, missing edges,
  // stale chat context) without this write.
  const flushCurrent = useCallback(() => {
    if (!canvasId) {
      return Promise.resolve(true);
    }
    return flush((value) => saveCanvas(canvasId, { title, document: value }));
  }, [canvasId, title, flush]);

  const runNode = useCallback(
    (nodeId: string) => {
      if (!canvasId) {
        return;
      }
      patchNodeData(nodeId, { status: "running", error: null });
      void flushCurrent().then((ok) => {
        if (!ok) {
          patchNodeData(nodeId, { status: "error", error: "could not save canvas" });
          return;
        }
        return runCanvasNode(canvasId, nodeId, {
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
      });
    },
    [canvasId, flushCurrent, patchNodeData],
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
      const position = suggestedPosition(payload) ?? nextNodePosition(prev);
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
            Select a canvas on the left, or create a new one.
          </div>
        )}
      </div>
      <ChatPanel canvasId={canvasId ?? ""} selectedNodeIds={selectedNodeIds} onSkillNode={addSkillNode} placementOrigin={placementOrigin(doc)} onBeforeSend={flushCurrent} />
    </div>
  );
}

const statusLabels: Record<SaveStatus, { text: string; className: string }> = {
  idle: { text: "", className: "text-muted-foreground" },
  pending: { text: "Unsaved...", className: "text-muted-foreground" },
  saving: { text: "Saving...", className: "text-muted-foreground" },
  saved: { text: "Saved", className: "text-emerald-600" },
  error: { text: "Save failed - Retry", className: "text-destructive" },
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
      <span className="truncate text-sm font-semibold">{title || "Untitled canvas"}</span>
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

function suggestedPosition(payload: SkillNodePayload): { x: number; y: number } | null {
  const { x, y } = payload;
  if (Number.isFinite(x) && Number.isFinite(y) && (x !== 0 || y !== 0)) {
    return { x, y };
  }
  return null;
}

function nextNodePosition(doc: CanvasDocument): { x: number; y: number } {
  const n = doc.nodes.length;
  return { x: 80 + (n % 6) * 48, y: 80 + (n % 6) * 48 };
}

// World coordinate near the top-left of the currently visible board (screen
// origin maps to world (-vp.x, -vp.y) / zoom), so AI-created nodes land in the
// user's view instead of cascading from the canvas origin.
function placementOrigin(doc: CanvasDocument | null): { x: number; y: number } {
  const vp = doc?.viewport ?? { x: 0, y: 0, zoom: 1 };
  const zoom = vp.zoom || 1;
  return { x: (-vp.x + 80) / zoom, y: (-vp.y + 80) / zoom };
}
