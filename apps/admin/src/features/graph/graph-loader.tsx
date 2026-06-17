import { useEffect } from "react";
import Graph from "graphology";
import { useLoadGraph, useSigma } from "@react-sigma/core";
import { COMMUNITY_COLORS, nodeColor, nodeSize, WORKER_LAYOUT_NODE_THRESHOLD } from "./graph-colors";
import {
  edgeVisibilityThreshold,
  graphDataKey,
  layoutIterations,
  makeLayoutWorker,
  runForceLayout,
  scalingRatioFor,
} from "./graph-layout";
import type { GraphEdge, GraphNode } from "./types";

export type ColorMode = "type" | "community";

const positionCache = new Map<string, { x: number; y: number }>();
let lastLayoutDataKey = "";
let pendingLayoutDataKey = "";

interface GraphLoaderProps {
  nodes: GraphNode[];
  edges: GraphEdge[];
  colorMode: ColorMode;
  nodeScale: number;
  graphSpacing: number;
}

export function GraphLoader({ nodes, edges, colorMode, nodeScale, graphSpacing }: GraphLoaderProps) {
  const loadGraph = useLoadGraph();
  const sigma = useSigma();

  useEffect(() => {
    const dataKey = graphDataKey(nodes, edges, graphSpacing);
    const needsLayout = dataKey !== lastLayoutDataKey && dataKey !== pendingLayoutDataKey;
    let cancelled = false;
    let worker: Worker | null = null;

    const graph = new Graph();
    const maxLinks = Math.max(...nodes.map((n) => n.linkCount), 1);
    const weakEdgeThreshold = edgeVisibilityThreshold(nodes.length);

    for (const node of nodes) {
      const cached = positionCache.get(node.id);
      const color =
        colorMode === "community"
          ? COMMUNITY_COLORS[node.community % COMMUNITY_COLORS.length]
          : nodeColor(node.type);
      graph.addNode(node.id, {
        type: "circle",
        x: cached?.x ?? Math.random() * 100,
        y: cached?.y ?? Math.random() * 100,
        size: nodeSize(node.linkCount, maxLinks, nodes.length, nodeScale),
        color,
        label: node.label,
        nodeType: node.type,
        nodePath: node.path,
        community: node.community,
      });
    }

    const maxWeight = Math.max(...edges.map((e) => e.weight), 1);

    for (const edge of edges) {
      if (graph.hasNode(edge.source) && graph.hasNode(edge.target)) {
        const edgeKey = `${edge.source}->${edge.target}`;
        if (!graph.hasEdge(edgeKey) && !graph.hasEdge(`${edge.target}->${edge.source}`)) {
          const normalizedWeight = edge.weight / maxWeight;
          const size = 0.5 + normalizedWeight * 3.5;
          const alpha = Math.round(40 + normalizedWeight * 180);
          const color = `rgba(100,116,139,${alpha / 255})`;
          graph.addEdgeWithKey(edgeKey, edge.source, edge.target, {
            color,
            size,
            weight: edge.weight,
            normalizedWeight,
            sourceNode: edge.source,
            targetNode: edge.target,
            lowPriority: weakEdgeThreshold > 0 && normalizedWeight < weakEdgeThreshold,
          });
        }
      }
    }

    const runMainThreadLayout = () => {
      runForceLayout(graph, nodes.length, graphSpacing);
      lastLayoutDataKey = dataKey;
      graph.forEachNode((nodeId, attrs) => {
        positionCache.set(nodeId, { x: attrs.x as number, y: attrs.y as number });
      });
    };

    if (needsLayout && nodes.length > 1 && nodes.length < WORKER_LAYOUT_NODE_THRESHOLD) {
      runMainThreadLayout();
    }

    loadGraph(graph);

    if (needsLayout && nodes.length >= WORKER_LAYOUT_NODE_THRESHOLD) {
      worker = makeLayoutWorker();
      if (!worker) {
        runMainThreadLayout();
        loadGraph(graph);
        return undefined;
      }
      pendingLayoutDataKey = dataKey;

      worker.onmessage = (
        event: MessageEvent<{ key: string; positions: Array<{ id: string; x: number; y: number }> }>,
      ) => {
        if (cancelled || event.data.key !== dataKey) return;
        for (const { id, x, y } of event.data.positions) {
          if (!graph.hasNode(id)) continue;
          graph.setNodeAttribute(id, "x", x);
          graph.setNodeAttribute(id, "y", y);
          positionCache.set(id, { x, y });
        }
        lastLayoutDataKey = dataKey;
        if (pendingLayoutDataKey === dataKey) pendingLayoutDataKey = "";
        sigma.refresh();
      };
      worker.onerror = (event) => {
        if (cancelled) return;
        console.warn("[Graph] layout worker failed; falling back to main-thread layout:", event.message);
        if (pendingLayoutDataKey === dataKey) pendingLayoutDataKey = "";
        runMainThreadLayout();
        loadGraph(graph);
      };
      worker.postMessage({
        key: dataKey,
        nodes: nodes.map((node) => {
          const cached = positionCache.get(node.id);
          return {
            id: node.id,
            x: cached?.x ?? (graph.getNodeAttribute(node.id, "x") as number),
            y: cached?.y ?? (graph.getNodeAttribute(node.id, "y") as number),
          };
        }),
        edges: edges.map((edge) => ({ source: edge.source, target: edge.target, weight: edge.weight })),
        iterations: layoutIterations(nodes.length),
        scalingRatio: scalingRatioFor(nodes.length, graphSpacing),
      });
    }

    return () => {
      cancelled = true;
      if (pendingLayoutDataKey === dataKey) pendingLayoutDataKey = "";
      worker?.terminate();
    };
  }, [loadGraph, sigma, nodes, edges, colorMode, nodeScale, graphSpacing]);

  return null;
}
