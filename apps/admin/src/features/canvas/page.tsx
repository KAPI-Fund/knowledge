import { Pencil } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useParams } from "react-router-dom";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { uuid } from "@/lib/uuid";

import { extractUrl, searchWeb } from "./api";
import { CanvasBoard } from "./canvas-board";
import { CanvasToolbar } from "./canvas-toolbar";
import { ChatPanel, type SkillNodePayload } from "./chat-panel";
import { HistorySidebar } from "./history-sidebar";
import { pendingSaveStore } from "./pending-save-store";
import { useCanvas, useCanvasCacheSave, useSaveCanvas } from "./queries";
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
  const cacheSave = useCanvasCacheSave();
  const [doc, setDoc] = useState<CanvasDocument | null>(null);
  const [title, setTitle] = useState("");
  const [selectedNodeIds, setSelectedNodeIds] = useState<string[]>([]);
  // The canvas list opens on each entry and collapses to a rail once the user
  // starts working in the board or chat, so it never steals width from the
  // workspace while editing.
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  // A leave-save that failed, kept under its own id so it can be retried after
  // the board has already been dropped. Without this the last edit to the
  // canvas being navigated away from would be lost with no way to recover it.
  const [failedSave, setFailedSave] = useState<{
    id: string;
    title: string;
    document: CanvasDocument;
  } | null>(null);
  const loadedId = useRef<string | undefined>(undefined);

  const onSave = useCallback(
    (value: CanvasDocument) => save.mutateAsync({ title, document: value }),
    [save, title],
  );

  const { status, reset, flush } = useAutosave({ value: doc ?? emptyDoc(), delayMs: 800, onSave });

  // Flush a pending edit to the currently loaded canvas's own id. `onSave` is
  // bound to the route id, which is wrong once the route has moved on, so we
  // save through the loaded id captured here instead. The failure sink differs
  // by exit path (see the two callers below), so it is passed in.
  const flushLoadedWith = useCallback(
    (
      onFail: (snapshot: {
        id: string;
        title: string;
        document: CanvasDocument;
        seq: number;
      }) => void,
    ) => {
      const staleId = loadedId.current;
      const staleTitle = title;
      if (!staleId) {
        return;
      }
      // Generation token for this leave-save. A successful save resolves any
      // queued retry for this canvas at or below this seq; a failure hands the
      // seq to onFail so the retry it enqueues is superseded once a newer save
      // for the same canvas lands (P1: no stale retry overwriting fresh edits).
      const seq = pendingSaveStore.allocateSeq();
      return flush((value) =>
        cacheSave(staleId, { title: staleTitle, document: value })
          .then((data) => {
            pendingSaveStore.resolve(staleId, seq);
            return data;
          })
          .catch((error: unknown) => {
            onFail({ id: staleId, title: staleTitle, document: value, seq });
            throw error;
          }),
      );
    },
    [flush, title, cacheSave],
  );

  // Same-instance route change (e.g. /canvas/c1 -> /canvas): the board is
  // dropped but the page stays mounted, so a failed leave-save can retain the
  // snapshot and surface the header Retry button.
  const flushLoaded = useCallback(
    () =>
      flushLoadedWith(({ id, title: t, document }) => setFailedSave({ id, title: t, document })),
    [flushLoadedWith],
  );

  // Leaving the canvas section entirely unmounts the page, so setFailedSave
  // would target an unmounted component and render no Retry affordance. Enqueue
  // the failed save into the persistent pending-save store instead; the
  // App-level PendingSaveBanner outlives this page and drains/retries it (and
  // it survives a reload so the edit is not lost with the tab).
  const flushLoadedOnUnmount = useCallback(
    () => flushLoadedWith((snapshot) => pendingSaveStore.enqueue(snapshot)),
    [flushLoadedWith],
  );

  const retryFailedSave = useCallback(() => {
    const pending = failedSave;
    if (!pending) {
      return;
    }
    void cacheSave(pending.id, { title: pending.title, document: pending.document })
      .then(() => setFailedSave(null))
      .catch(() => setFailedSave(pending));
  }, [failedSave, cacheSave]);

  // A same-instance route change runs the load effect's clear branch; leaving
  // the canvas section entirely unmounts this component and only runs effect
  // cleanups. Flush on unmount too so the last in-debounce edit is not lost --
  // via the persistent-queue variant, since no Retry button can render here.
  // The ref keeps the [] cleanup pinned to the latest loaded id/title/doc.
  const flushLoadedRef = useRef(flushLoadedOnUnmount);
  flushLoadedRef.current = flushLoadedOnUnmount;
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
        // Reopening a canvas cancels any pending leave-retry for it: the user is
        // now editing the freshly loaded server copy, so an older queued edit
        // must not later overwrite it (P1). Any entry at or below the current
        // seq predates this reopen and is superseded.
        pendingSaveStore.resolve(loaded.id, pendingSaveStore.peekSeq());
        setDoc(loaded.document);
        setTitle(loaded.title);
        setSelectedNodeIds([]);
        setSidebarCollapsed(false);
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
    return flush((value) => cacheSave(canvasId, { title, document: value }));
  }, [canvasId, title, flush, cacheSave]);

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
                      // Keep at most the 10 most recent versions; each Run/Regenerate
                      // appends one and they would otherwise grow the saved document
                      // without bound.
                      versions: [...versions, version].slice(-10),
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

  const runSearchNode = useCallback(
    (nodeId: string) => {
      const node = doc?.nodes.find((n) => n.id === nodeId);
      const query = typeof node?.data.query === "string" ? node.data.query : "";
      if (!query) {
        return;
      }
      patchNodeData(nodeId, { status: "loading", error: null });
      void searchWeb(query)
        .then((result) => {
          if (result.status === "ok") {
            patchNodeData(nodeId, {
              status: "idle",
              error: null,
              markdown: result.markdown,
            });
          } else {
            patchNodeData(nodeId, { status: "error", error: result.error ?? "search failed" });
          }
        })
        .catch((error: unknown) => {
          patchNodeData(nodeId, {
            status: "error",
            error: error instanceof Error ? error.message : "search failed",
          });
        });
    },
    [doc, patchNodeData],
  );

  // Rename persists on its own: autosave only watches the document, so a
  // title-only change would never be written. Push it straight through the
  // cache-synced save so the header, list, and server agree immediately.
  const renameCanvas = useCallback(
    (nextTitle: string) => {
      const trimmed = nextTitle.trim();
      if (trimmed === title) {
        return;
      }
      setTitle(trimmed);
      if (!canvasId || !doc) {
        return;
      }
      void cacheSave(canvasId, { title: trimmed, document: doc });
    },
    [title, canvasId, doc, cacheSave],
  );

  const addSkillNode = useCallback((payload: SkillNodePayload) => {
    const source = payload.node as { type: CanvasNode["type"]; data?: Record<string, unknown> };
    const id = uuid();
    setDoc((prev) => {
      if (!prev) {
        return prev;
      }
      const size = defaultSize(source.type);
      // Chain placement: every new node sits to the right of the most recently
      // created node (highest creation number), top-aligned. The first node on
      // an empty canvas falls back to the payload/view origin.
      const latest = latestNode(prev.nodes);
      const position = latest
        ? { x: latest.x + latest.w + CHAIN_GAP, y: latest.y }
        : (suggestedPosition(payload) ?? placementOrigin(prev));
      const node: CanvasNode = {
        id,
        type: source.type,
        x: position.x,
        y: position.y,
        w: size.w,
        h: size.h,
        // Stable creation number, assigned once and never renumbered (gaps are
        // left after deletions). Read back for display via data.index.
        data: { ...(source.data ?? {}), index: nextNodeIndex(prev.nodes) },
      };
      const existingIds = new Set(prev.nodes.map((n) => n.id));
      const sourceIds = Array.isArray(source.data?.sourceNodeIds)
        ? (source.data?.sourceNodeIds as unknown[]).filter(
            (s): s is string => typeof s === "string" && existingIds.has(s),
          )
        : [];
      // Auto-connect the new node to the latest node, merged with any explicit
      // source references and de-duplicated so the chain predecessor doubling as
      // a source still yields a single edge.
      const linkSources = new Set(sourceIds);
      if (latest) {
        linkSources.add(latest.id);
      }
      const newEdges = [...linkSources].map((src) => ({
        id: uuid(),
        source: src,
        target: id,
      }));
      return { ...prev, nodes: [...prev.nodes, node], edges: [...prev.edges, ...newEdges] };
    });
  }, []);

  return (
    <div className="flex h-full min-h-0">
      <HistorySidebar
        activeId={canvasId}
        collapsed={sidebarCollapsed}
        onToggle={() => setSidebarCollapsed((value) => !value)}
      />
      <div
        className="flex min-w-0 flex-1"
        onPointerDownCapture={() => setSidebarCollapsed(true)}
        onFocusCapture={() => setSidebarCollapsed(true)}
      >
        <div className="flex min-w-0 flex-1 flex-col">
          <CanvasHeader title={title} status={status} hasDoc={!!doc} onRename={renameCanvas} onRetry={() => doc && void onSave(doc)} failedSaveTitle={failedSave?.title} onRetryFailed={retryFailedSave} actions={doc ? <CanvasToolbar onAdd={addSkillNode} /> : null} />
          {doc ? (
            <div className="min-h-0 flex-1">
              <CanvasBoard key={canvasId} document={doc} onChange={setDoc} onRunNode={runNode} onFetchUrl={fetchUrlNode} onSearchNode={runSearchNode} onSelectionChange={setSelectedNodeIds} />
            </div>
          ) : (
            <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
              Select a canvas on the left, or create a new one.
            </div>
          )}
        </div>
        <ChatPanel canvasId={canvasId ?? ""} selectedNodeIds={selectedNodeIds} onSkillNode={addSkillNode} placementOrigin={placementOrigin(doc)} onBeforeSend={flushCurrent} />
      </div>
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
  hasDoc: boolean;
  onRename: (title: string) => void;
  onRetry: () => void;
  failedSaveTitle?: string;
  onRetryFailed: () => void;
  actions?: ReactNode;
}

function CanvasHeader({
  title,
  status,
  hasDoc,
  onRename,
  onRetry,
  failedSaveTitle,
  onRetryFailed,
  actions,
}: CanvasHeaderProps) {
  const label = statusLabels[status];
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(title);

  const beginEdit = () => {
    setDraft(title);
    setEditing(true);
  };

  const commit = () => {
    setEditing(false);
    onRename(draft);
  };

  return (
    <header className="flex items-center justify-between gap-2 border-b px-4 py-2">
      {editing ? (
        <Input
          autoFocus
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={commit}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.currentTarget.blur();
            } else if (event.key === "Escape") {
              setEditing(false);
            }
          }}
          className="h-7 max-w-xs text-sm font-semibold"
          aria-label="Canvas title"
        />
      ) : (
        <div className="flex min-w-0 items-center gap-1">
          <span className="truncate text-sm font-semibold">{title || "Untitled canvas"}</span>
          {hasDoc ? (
            <Button
              type="button"
              size="icon-xs"
              variant="ghost"
              onClick={beginEdit}
              aria-label="Rename canvas"
              className="shrink-0 text-muted-foreground hover:text-foreground"
            >
              <Pencil className="size-3.5" />
            </Button>
          ) : null}
        </div>
      )}
      <div className="flex shrink-0 items-center gap-3">
        {actions}
        {failedSaveTitle !== undefined ? (
          <button type="button" onClick={onRetryFailed} className="text-xs text-destructive">
            {`Couldn't save "${failedSaveTitle || "Untitled canvas"}" - Retry`}
          </button>
        ) : null}
        {hasDoc && label.text ? (
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

// Horizontal gap between a node and the next one chained to its right.
const CHAIN_GAP = 40;

// A node's effective creation number: the stored stable index, or its array
// position for legacy nodes saved before numbering existed.
function effectiveIndex(node: CanvasNode, position: number): number {
  return typeof node.data.index === "number" ? node.data.index : position + 1;
}

// The most recently created node (highest effective number). New nodes chain to
// its right, so keying on the number is robust to array reordering.
function latestNode(nodes: CanvasNode[]): CanvasNode | null {
  let best: CanvasNode | null = null;
  let bestIndex = -Infinity;
  nodes.forEach((node, position) => {
    const idx = effectiveIndex(node, position);
    if (idx >= bestIndex) {
      bestIndex = idx;
      best = node;
    }
  });
  return best;
}

// The next stable creation number: above every stored index (so gaps from
// deletions are preserved) and above the node count (so it never collides with a
// legacy node's position-based fallback number).
function nextNodeIndex(nodes: CanvasNode[]): number {
  let maxStored = 0;
  for (const node of nodes) {
    if (typeof node.data.index === "number" && node.data.index > maxStored) {
      maxStored = node.data.index;
    }
  }
  return Math.max(maxStored, nodes.length) + 1;
}

// Per-type starting sizes tuned to each node's typical content. Users can resize
// freely afterwards (persisted), so these are just sensible defaults, not caps.
const NODE_SIZES: Record<CanvasNode["type"], { w: number; h: number }> = {
  // A single freeform markdown textarea: a comfortable, slightly-tall writing area.
  note: { w: 300, h: 220 },
  // URL input + fetched article markdown; content-heavy, so give the body reading room.
  url: { w: 360, h: 340 },
  // Query input + web-search results markdown; same content-heavy shape as url.
  search: { w: 360, h: 340 },
  // Read-only reference showing just a project name on one line; keep it compact.
  kb: { w: 260, h: 120 },
  // Prompt + a long streamed markdown answer; the answer dominates, so run tall.
  ai_analyze: { w: 380, h: 360 },
  // Prompt on top + a (default square 1024x1024) generated image filling the rest.
  ai_image: { w: 340, h: 420 },
};

function defaultSize(type: CanvasNode["type"]): { w: number; h: number } {
  return NODE_SIZES[type] ?? { w: 280, h: 160 };
}

// World coordinate near the top-left of the currently visible board (screen
// origin maps to world (-vp.x, -vp.y) / zoom), so AI-created nodes land in the
// user's view instead of cascading from the canvas origin.
function placementOrigin(doc: CanvasDocument | null): { x: number; y: number } {
  const vp = doc?.viewport ?? { x: 0, y: 0, zoom: 1 };
  const zoom = vp.zoom || 1;
  return { x: (-vp.x + 80) / zoom, y: (-vp.y + 80) / zoom };
}
