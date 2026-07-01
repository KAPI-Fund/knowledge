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
import { useCallback, useMemo } from "react";

import { AiAnalyzeNode, type AiAnalyzeNodeData } from "./node-types/ai-analyze";
import { AiImageNode, type AiImageNodeData } from "./node-types/ai-image";
import { KbNode, type KbNodeData } from "./node-types/kb";
import { NoteNode, type NoteNodeData } from "./node-types/note";
import { UrlNode, type UrlNodeData } from "./node-types/url";
import type { CanvasDocument } from "./types";

interface NodeCallbacks {
  onPatch: (patch: Record<string, unknown>) => void;
  onRun: () => void;
}

function callbacks(data: Record<string, unknown>): NodeCallbacks {
  return (data.__cb as NodeCallbacks | undefined) ?? { onPatch: () => {}, onRun: () => {} };
}

function NoteAdapter({ data }: NodeProps) {
  const cb = callbacks(data);
  return (
    <NoteNode
      data={data as unknown as NoteNodeData}
      onChange={(markdown) => cb.onPatch({ markdown })}
    />
  );
}

function UrlAdapter({ data }: NodeProps) {
  const cb = callbacks(data);
  return (
    <UrlNode
      data={data as unknown as UrlNodeData}
      onUrlChange={(url) => cb.onPatch({ url })}
      onFetch={cb.onRun}
    />
  );
}

function KbAdapter({ data }: NodeProps) {
  return <KbNode data={data as unknown as KbNodeData} />;
}

function AnalyzeAdapter({ data }: NodeProps) {
  const cb = callbacks(data);
  return (
    <AiAnalyzeNode
      data={data as unknown as AiAnalyzeNodeData}
      onRerun={cb.onRun}
      onVersionChange={(id) => cb.onPatch({ activeVersionId: id })}
    />
  );
}

function ImageAdapter({ data }: NodeProps) {
  const cb = callbacks(data);
  return (
    <AiImageNode
      data={data as unknown as AiImageNodeData}
      onRegenerate={cb.onRun}
      onVersionChange={(id) => cb.onPatch({ activeVersionId: id })}
    />
  );
}

const nodeTypes: NodeTypes = {
  note: NoteAdapter,
  url: UrlAdapter,
  kb: KbAdapter,
  ai_analyze: AnalyzeAdapter,
  ai_image: ImageAdapter,
};

interface CanvasBoardProps {
  document: CanvasDocument;
  onChange: (next: CanvasDocument) => void;
  onRunNode: (nodeId: string) => void;
}

export function CanvasBoard({ document, onChange, onRunNode }: CanvasBoardProps) {
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

  const rfNodes = useMemo<Node[]>(
    () =>
      document.nodes.map((n) => ({
        id: n.id,
        type: n.type,
        position: { x: n.x, y: n.y },
        data: {
          ...n.data,
          __cb: {
            onPatch: (patch: Record<string, unknown>) => patchNode(n.id, patch),
            onRun: () => onRunNode(n.id),
          } satisfies NodeCallbacks,
        },
      })),
    [document.nodes, patchNode, onRunNode],
  );

  const rfEdges = useMemo<Edge[]>(
    () => document.edges.map((e) => ({ id: e.id, source: e.source, target: e.target })),
    [document.edges],
  );

  const onNodesChange = useCallback(
    (changes: NodeChange[]) => {
      const next = applyNodeChanges(changes, rfNodes);
      onChange({
        ...document,
        nodes: next
          .map((n) => {
            const orig = document.nodes.find((d) => d.id === n.id);
            return orig ? { ...orig, x: n.position.x, y: n.position.y } : null;
          })
          .filter((n): n is (typeof document.nodes)[number] => n !== null),
      });
    },
    [document, onChange, rfNodes],
  );

  const onEdgesChange = useCallback(
    (changes: EdgeChange[]) => {
      const next = applyEdgeChanges(changes, rfEdges);
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
          fitView
        >
          <Background />
          <Controls />
        </ReactFlow>
      </ReactFlowProvider>
    </div>
  );
}
