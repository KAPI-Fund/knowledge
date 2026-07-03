import "@xyflow/react/dist/style.css";

import {
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
  Background,
  Controls,
  ReactFlow,
  ReactFlowProvider,
  type Connection,
  type Edge,
  type EdgeChange,
  type Node,
  type NodeChange,
  type NodeProps,
  type NodeTypes,
} from "@xyflow/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useSystemSettingsQuery } from "../settings/queries";
import { AiAnalyzeNode, type AiAnalyzeNodeData } from "./node-types/ai-analyze";
import { AiImageNode, type AiImageNodeData } from "./node-types/ai-image";
import { KbNode, type KbNodeData } from "./node-types/kb";
import { NoteNode, type NoteNodeData } from "./node-types/note";
import { SearchNode, type SearchNodeData } from "./node-types/search";
import { UrlNode, type UrlNodeData } from "./node-types/url";
import type { CanvasDocument } from "./types";

interface NodeCallbacks {
  onPatch: (patch: Record<string, unknown>) => void;
  onRun: () => void;
  onFetchUrl: () => void;
  onSearch: () => void;
}

function callbacks(data: Record<string, unknown>): NodeCallbacks {
  return (
    (data.__cb as NodeCallbacks | undefined) ?? {
      onPatch: () => {},
      onRun: () => {},
      onFetchUrl: () => {},
      onSearch: () => {},
    }
  );
}

function modelOf(data: Record<string, unknown>): string | null {
  return (data.__model as string | null | undefined) ?? null;
}

function indexOf(data: Record<string, unknown>): number | undefined {
  return typeof data.__index === "number" ? data.__index : undefined;
}

function NoteAdapter({ id, data, selected }: NodeProps) {
  const cb = callbacks(data);
  return (
    <NoteNode
      data={data as unknown as NoteNodeData}
      nodeId={id}
      index={indexOf(data)}
      selected={selected}
      onChange={(markdown) => cb.onPatch({ markdown })}
    />
  );
}

function UrlAdapter({ id, data, selected }: NodeProps) {
  const cb = callbacks(data);
  return (
    <UrlNode
      data={data as unknown as UrlNodeData}
      nodeId={id}
      index={indexOf(data)}
      selected={selected}
      onUrlChange={(url) => cb.onPatch({ url })}
      onFetch={cb.onFetchUrl}
    />
  );
}

function SearchAdapter({ id, data, selected }: NodeProps) {
  const cb = callbacks(data);
  return (
    <SearchNode
      data={data as unknown as SearchNodeData}
      nodeId={id}
      index={indexOf(data)}
      selected={selected}
      onQueryChange={(query) => cb.onPatch({ query })}
      onSearch={cb.onSearch}
    />
  );
}

function KbAdapter({ id, data, selected }: NodeProps) {
  return (
    <KbNode data={data as unknown as KbNodeData} nodeId={id} index={indexOf(data)} selected={selected} />
  );
}

function AnalyzeAdapter({ id, data, selected }: NodeProps) {
  const cb = callbacks(data);
  return (
    <AiAnalyzeNode
      data={data as unknown as AiAnalyzeNodeData}
      nodeId={id}
      index={indexOf(data)}
      selected={selected}
      model={modelOf(data)}
      onRerun={cb.onRun}
      onVersionChange={(vid) => cb.onPatch({ activeVersionId: vid })}
      onPromptChange={(prompt) => cb.onPatch({ prompt })}
    />
  );
}

function ImageAdapter({ id, data, selected }: NodeProps) {
  const cb = callbacks(data);
  return (
    <AiImageNode
      data={data as unknown as AiImageNodeData}
      nodeId={id}
      index={indexOf(data)}
      selected={selected}
      model={modelOf(data)}
      onRegenerate={cb.onRun}
      onVersionChange={(vid) => cb.onPatch({ activeVersionId: vid })}
      onPromptChange={(prompt) => cb.onPatch({ prompt })}
    />
  );
}

const nodeTypes: NodeTypes = {
  note: NoteAdapter,
  url: UrlAdapter,
  search: SearchAdapter,
  kb: KbAdapter,
  ai_analyze: AnalyzeAdapter,
  ai_image: ImageAdapter,
};

interface CanvasBoardProps {
  document: CanvasDocument;
  onChange: (next: CanvasDocument) => void;
  onRunNode: (nodeId: string) => void;
  onFetchUrl: (nodeId: string) => void;
  onSearchNode: (nodeId: string) => void;
  onSelectionChange?: (nodeIds: string[]) => void;
}

// Drop edges whose source or target no longer exists. React Flow's default
// Backspace-delete removes a node through onNodesChange but leaves its edges
// behind, so without this a deleted node's edges would be persisted as dangling
// references (and re-rendered as edges to nowhere on reload).
export function pruneDanglingEdges(
  nodes: CanvasDocument["nodes"],
  edges: CanvasDocument["edges"],
): CanvasDocument["edges"] {
  const ids = new Set(nodes.map((n) => n.id));
  return edges.filter((e) => ids.has(e.source) && ids.has(e.target));
}

export function CanvasBoard({ document, onChange, onRunNode, onFetchUrl, onSearchNode, onSelectionChange }: CanvasBoardProps) {
  const model = useSystemSettingsQuery().data?.providerModel ?? null;
  // We rebuild rfNodes from the document on every change, which discards React
  // Flow's internal `selected` flag. Track the selection here and re-stamp it so
  // the selected node keeps its outline; a pane click clears it (deselect).
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const patchNode = useCallback(
    (nodeId: string, patch: Record<string, unknown>) => {
      onChange({
        ...document,
        nodes: document.nodes.map((n) =>
          n.id === nodeId ? { ...n, data: { ...n.data, ...patch } } : n,
        ),
      });
    },
    [document, onChange],
  );

  // Nodes derived from the document model (positions, sizes, injected data). This
  // is the source of truth for structure/data, but NOT for the live drag: while
  // dragging, React Flow owns the position and we must not overwrite it.
  const documentNodes = useMemo<Node[]>(
    () =>
      document.nodes.map((n, i) => ({
        id: n.id,
        type: n.type,
        position: { x: n.x, y: n.y },
        selected: selectedIds.has(n.id),
        // React Flow gates node visibility on nodeHasDimensions (measured ??
        // width ?? initialWidth). We reconstruct nodes from the document model on
        // every change, so the measured size React Flow reports back through
        // onNodesChange is discarded and reset -- leaving nodes `visibility:
        // hidden` forever. Supplying the model's own size keeps them visible and
        // gives the h-full node bodies a box to fill.
        width: n.w,
        height: n.h,
        data: {
          ...n.data,
          // Stable creation number when present; array position is only a
          // fallback for legacy nodes saved before numbering existed.
          __index: typeof n.data.index === "number" ? n.data.index : i + 1,
          __model: model,
          __cb: {
            onPatch: (patch: Record<string, unknown>) => patchNode(n.id, patch),
            onRun: () => onRunNode(n.id),
            onFetchUrl: () => onFetchUrl(n.id),
            onSearch: () => onSearchNode(n.id),
          } satisfies NodeCallbacks,
        },
      })),
    [document.nodes, patchNode, onRunNode, onFetchUrl, onSearchNode, model, selectedIds],
  );

  // React Flow's live node state. It owns positions during a drag so nodes follow
  // the cursor frame-by-frame; the document is only updated on drag end. Outside a
  // drag we mirror documentNodes so external edits (new nodes, run results, loads)
  // flow in. Overwriting mid-drag would reset the position and make the node snap
  // to its committed spot only on mouse-up.
  const [rfNodes, setRfNodes] = useState<Node[]>(documentNodes);
  const draggingRef = useRef(false);

  useEffect(() => {
    if (draggingRef.current) {
      return;
    }
    setRfNodes(documentNodes);
  }, [documentNodes]);

  const documentEdges = useMemo<Edge[]>(
    () => document.edges.map((e) => ({ id: e.id, source: e.source, target: e.target })),
    [document.edges],
  );

  // React Flow owns edge selection, mirroring the node handling. Rebuilding edges
  // from the document every render would wipe the `selected` flag, so a clicked
  // edge would be deselected before a Delete keypress could remove it. Keep edge
  // state here and only commit structural changes (connections, deletions).
  const [rfEdges, setRfEdges] = useState<Edge[]>(documentEdges);

  useEffect(() => {
    setRfEdges(documentEdges);
  }, [documentEdges]);

  const onNodesChange = useCallback(
    (changes: NodeChange[]) => {
      // Apply every change to React Flow's own node objects so drag deltas and
      // measured sizes persist across renders (this is what makes the node track
      // the cursor).
      const next = applyNodeChanges(changes, rfNodes);
      setRfNodes(next);

      // Selection is view-only state: React Flow emits a `select` change on every
      // click. Persisting it through onChange would churn the document and loop
      // (React #185), so mirror it into local state and notify the parent instead.
      const ids = next.filter((n) => n.selected).map((n) => n.id);
      if (!(selectedIds.size === ids.length && ids.every((id) => selectedIds.has(id)))) {
        setSelectedIds(new Set(ids));
        onSelectionChange?.(ids);
      }

      // Mid-drag: keep the live position in React Flow only; committing to the
      // document now would rebuild documentNodes and fight the drag.
      const dragging = changes.some((c) => c.type === "position" && c.dragging);
      draggingRef.current = dragging;
      if (dragging) {
        return;
      }

      // Commit only structural changes (drag end, deletions) to the document. A
      // batch that is purely selection or measurement leaves the document alone.
      if (!changes.some((c) => c.type === "position" || c.type === "remove")) {
        return;
      }
      const nodes = next
        .map((n) => {
          const orig = document.nodes.find((d) => d.id === n.id);
          return orig ? { ...orig, x: n.position.x, y: n.position.y } : null;
        })
        .filter((n): n is (typeof document.nodes)[number] => n !== null);
      onChange({ ...document, nodes, edges: pruneDanglingEdges(nodes, document.edges) });
    },
    [document, onChange, rfNodes, selectedIds, onSelectionChange],
  );

  const onEdgesChange = useCallback(
    (changes: EdgeChange[]) => {
      const next = applyEdgeChanges(changes, rfEdges);
      setRfEdges(next);
      // Selection is view-only; only structural changes (deletions) reach the
      // document, so a plain edge click does not churn state.
      if (!changes.some((c) => c.type !== "select")) {
        return;
      }
      onChange({
        ...document,
        edges: next.map((e) => ({ id: e.id, source: e.source, target: e.target })),
      });
    },
    [document, onChange, rfEdges],
  );

  const onConnect = useCallback(
    (conn: Connection) => {
      const next = addEdge(conn, rfEdges);
      setRfEdges(next);
      onChange({
        ...document,
        edges: next.map((e) => ({ id: e.id, source: e.source, target: e.target })),
      });
    },
    [document, onChange, rfEdges],
  );

  return (
    <div className="h-full w-full">
      <ReactFlowProvider>
        <ReactFlow
          nodes={rfNodes}
          edges={rfEdges}
          nodeTypes={nodeTypes}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          deleteKeyCode={["Delete", "Backspace"]}
          defaultViewport={document.viewport}
        >
          <Background />
          <Controls />
        </ReactFlow>
      </ReactFlowProvider>
    </div>
  );
}
