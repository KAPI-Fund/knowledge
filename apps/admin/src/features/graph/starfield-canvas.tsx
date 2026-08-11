import { useEffect, useMemo, useRef } from "react";
import ForceGraph3D, { type ForceGraph3DInstance } from "3d-force-graph";
import { useTheme } from "next-themes";
import { Vector2, Vector3 } from "three";
import { UnrealBloomPass } from "three/examples/jsm/postprocessing/UnrealBloomPass.js";

import { COMMUNITY_COLORS, hexToRgba, nodeColor as nodeTypeColor } from "./graph-colors";
import {
  calculateHighlightChain,
  computeDepthLayers,
  edgeKey,
  EMPTY_CHAIN,
  type HighlightChain,
} from "./starfield-chains";
import {
  applyLayoutForces,
  applyLayoutNodeStates,
  type StarfieldLayout,
  type StarfieldNode,
} from "./starfield-forces";
import type { ColorMode } from "./graph-loader";
import type { GraphEdge, GraphNode } from "./types";

const UPSTREAM_COLOR = "#3b82f6";
const DOWNSTREAM_COLOR = "#ef4444";
const BACKGROUND_DARK = "#030712";
const BACKGROUND_LIGHT = "#f3f4f6";
const INITIAL_CAMERA = { x: 0, y: 300, z: 1300 };
const AUTO_ROTATE_SPEED = 0.0012;
const MAX_PROJECTED_LABELS = 40;

interface StarfieldLink {
  source: string | StarfieldNode;
  target: string | StarfieldNode;
  sourceId: string;
  targetId: string;
  key: string;
  weight: number;
}

type StarfieldGraph = ForceGraph3DInstance<StarfieldNode, StarfieldLink>;

interface VisualState {
  chain: HighlightChain;
  selectedId: string | null;
  dark: boolean;
  colorMode: ColorMode;
  highlighted: Set<string>;
}

function escapeHtml(value: string): string {
  return value.replace(
    /[&<>"']/g,
    (char) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char] as string,
  );
}

function baseNodeColor(node: StarfieldNode, colorMode: ColorMode): string {
  return colorMode === "community"
    ? (COMMUNITY_COLORS[node.community % COMMUNITY_COLORS.length] ?? nodeTypeColor(node.type))
    : nodeTypeColor(node.type);
}

export interface StarfieldCanvasProps {
  nodes: GraphNode[];
  edges: GraphEdge[];
  colorMode: ColorMode;
  layout: StarfieldLayout;
  selectedNodeId: string | null;
  highlightedNodes: Set<string>;
  onNodeClick: (nodeId: string) => void;
  onNodeContextMenu?: (nodeId: string, x: number, y: number) => void;
  onBackgroundClick?: () => void;
}

export function StarfieldCanvas({
  nodes,
  edges,
  colorMode,
  layout,
  selectedNodeId,
  highlightedNodes,
  onNodeClick,
  onNodeContextMenu,
  onBackgroundClick,
}: StarfieldCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const labelsRef = useRef<HTMLDivElement>(null);
  const graphRef = useRef<StarfieldGraph | null>(null);
  const nodeCacheRef = useRef(new Map<string, StarfieldNode>());
  const labelEntriesRef = useRef<Array<{ node: StarfieldNode; el: HTMLDivElement }>>([]);
  const bloomRef = useRef<UnrealBloomPass | null>(null);
  const visualsRef = useRef<VisualState>({
    chain: EMPTY_CHAIN,
    selectedId: null,
    dark: true,
    colorMode,
    highlighted: highlightedNodes,
  });
  const callbacksRef = useRef({ onNodeClick, onNodeContextMenu, onBackgroundClick });
  callbacksRef.current = { onNodeClick, onNodeContextMenu, onBackgroundClick };

  const { resolvedTheme } = useTheme();
  const dark = resolvedTheme !== "light";

  const chain = useMemo(
    () => (selectedNodeId ? calculateHighlightChain(selectedNodeId, edges) : EMPTY_CHAIN),
    [selectedNodeId, edges],
  );

  // Instance lifecycle: create once per mount, destroy on unmount.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return undefined;

    const graph = new ForceGraph3D(container) as unknown as StarfieldGraph;
    graphRef.current = graph;

    graph
      .width(container.clientWidth)
      .height(container.clientHeight)
      .showNavInfo(false)
      .nodeOpacity(0.9)
      .linkOpacity(1)
      .nodeVal((node) => 2.5 + Math.sqrt(node.linkCount) * 0.8)
      .nodeLabel((node) => {
        const accent = baseNodeColor(node, visualsRef.current.colorMode);
        return `<div style="background: rgba(10,15,30,0.85); padding: 8px 12px; border-radius: 8px; border: 1px solid rgba(255,255,255,0.15); color: #fff; pointer-events: none;">
          <div style="font-size: 11px; color: ${accent}; font-weight: 600; margin-bottom: 2px; white-space: nowrap;">${escapeHtml(node.type)} &middot; ${node.linkCount} links</div>
          <div style="font-size: 14px; font-weight: 600; white-space: nowrap;">${escapeHtml(node.label)}</div>
        </div>`;
      })
      .nodeColor((node) => {
        const v = visualsRef.current;
        const base = baseNodeColor(node, v.colorMode);
        if (v.selectedId) {
          if (node.id === v.selectedId) return v.dark ? "#ffffff" : "#111827";
          if (v.chain.upstream.has(node.id)) return UPSTREAM_COLOR;
          if (v.chain.downstream.has(node.id)) return DOWNSTREAM_COLOR;
          return hexToRgba(base, v.dark ? 0.1 : 0.15);
        }
        if (v.highlighted.size > 0 && !v.highlighted.has(node.id)) {
          return hexToRgba(base, v.dark ? 0.15 : 0.2);
        }
        return base;
      })
      .linkWidth((link) => {
        const v = visualsRef.current;
        if (v.selectedId) return v.chain.links.has(link.key) ? 2.5 : 0.2;
        return 0.8;
      })
      .linkColor((link) => {
        const v = visualsRef.current;
        if (v.selectedId) {
          if (v.chain.links.has(link.key)) {
            const sourceInUp = v.chain.upstream.has(link.sourceId) || link.sourceId === v.selectedId;
            const targetInUp = v.chain.upstream.has(link.targetId) || link.targetId === v.selectedId;
            return sourceInUp && targetInUp ? UPSTREAM_COLOR : DOWNSTREAM_COLOR;
          }
          return v.dark ? "rgba(255,255,255,0.01)" : "rgba(0,0,0,0.015)";
        }
        return v.dark ? "rgba(255,255,255,0.04)" : "rgba(0,0,0,0.06)";
      })
      .linkDirectionalParticles((link) => {
        const v = visualsRef.current;
        return v.selectedId && v.chain.links.has(link.key) ? 3 : 0;
      })
      .linkDirectionalParticleWidth((link) => {
        const v = visualsRef.current;
        return v.selectedId && v.chain.links.has(link.key) ? 2 : 0;
      })
      .linkDirectionalParticleSpeed(0.015)
      .onNodeClick((node) => {
        focusCameraOnNode(graph, node);
        callbacksRef.current.onNodeClick(node.id);
      })
      .onNodeRightClick((node, event) => {
        callbacksRef.current.onNodeContextMenu?.(node.id, event.clientX, event.clientY);
      })
      .onBackgroundClick(() => {
        callbacksRef.current.onBackgroundClick?.();
      });

    graph.cameraPosition(INITIAL_CAMERA);

    // Auto-rotation loop, port of os-taxonomy. Radius and angle are both
    // derived from the camera's current XZ projection each frame, so wheel
    // zoom, drags, and post-focus deselection all resume without snapping.
    const interaction = { isUserInteracting: false };
    const controls = graph.controls() as {
      addEventListener: (type: string, handler: () => void) => void;
    };
    controls.addEventListener("start", () => {
      interaction.isUserInteracting = true;
    });
    controls.addEventListener("end", () => {
      interaction.isUserInteracting = false;
    });

    let frame = 0;
    const animate = () => {
      frame = requestAnimationFrame(animate);
      const v = visualsRef.current;
      if (!v.selectedId) {
        if (!interaction.isUserInteracting) {
          const pos = graph.cameraPosition();
          const rXZ = Math.hypot(pos.x, pos.z) || 1e-6;
          const angle = Math.atan2(pos.x, pos.z) + AUTO_ROTATE_SPEED;
          graph.cameraPosition(
            { x: rXZ * Math.sin(angle), y: pos.y, z: rXZ * Math.cos(angle) },
            undefined,
            0,
          );
        }
      } else {
        projectLabels(graph, labelEntriesRef.current);
      }
    };
    animate();

    const observer = new ResizeObserver(() => {
      graph.width(container.clientWidth).height(container.clientHeight);
    });
    observer.observe(container);

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      bloomRef.current = null;
      graph._destructor();
      graphRef.current = null;
    };
  }, []);

  // Data + layout ingestion.
  useEffect(() => {
    const graph = graphRef.current;
    if (!graph) return;

    const cache = nodeCacheRef.current;
    const starNodes = nodes.map((node) => {
      const cached = cache.get(node.id);
      if (cached) {
        Object.assign(cached, node);
        return cached;
      }
      const created: StarfieldNode = { ...node };
      cache.set(node.id, created);
      return created;
    });

    const { layers, maxLayer } = computeDepthLayers(starNodes, edges);
    for (const node of starNodes) node.layer = layers.get(node.id) ?? 0;

    const links: StarfieldLink[] = edges.map((edge) => ({
      source: edge.source,
      target: edge.target,
      sourceId: edge.source,
      targetId: edge.target,
      key: edgeKey(edge),
      weight: edge.weight,
    }));

    applyLayoutNodeStates(starNodes, layout, maxLayer);
    graph.graphData({ nodes: starNodes, links });
    applyLayoutForces(graph, layout);
    // No d3ReheatSimulation() here: kapsule applies graphData on a debounced
    // (async) update pass that itself re-heats the simulation. Reheating
    // synchronously flips engineRunning on before that pass assigns
    // state.layout, and the first animation frame then crashes the render
    // loop with "Cannot read properties of undefined (reading 'tick')".
  }, [nodes, edges, layout]);

  // Visual state: selection chain, insights highlight, color mode, theme.
  useEffect(() => {
    const graph = graphRef.current;
    if (!graph) return;

    visualsRef.current = { chain, selectedId: selectedNodeId, dark, colorMode, highlighted: highlightedNodes };

    graph.backgroundColor(dark ? BACKGROUND_DARK : BACKGROUND_LIGHT);
    graph.nodeColor(graph.nodeColor());
    graph.linkWidth(graph.linkWidth());
    graph.linkColor(graph.linkColor());
    graph.linkDirectionalParticles(graph.linkDirectionalParticles());
    graph.linkDirectionalParticleWidth(graph.linkDirectionalParticleWidth());

    const composer = graph.postProcessingComposer();
    if (dark && !bloomRef.current) {
      const container = containerRef.current;
      const pass = new UnrealBloomPass(
        new Vector2(container?.clientWidth ?? 800, container?.clientHeight ?? 600),
        0.6,
        0.4,
        0.1,
      );
      composer.addPass(pass);
      bloomRef.current = pass;
    } else if (!dark && bloomRef.current) {
      composer.removePass(bloomRef.current);
      bloomRef.current = null;
    }

    rebuildLabels();
  }, [chain, selectedNodeId, dark, colorMode, highlightedNodes]);

  function rebuildLabels() {
    const container = labelsRef.current;
    if (!container) return;
    container.innerHTML = "";
    labelEntriesRef.current = [];
    if (!selectedNodeId) return;

    const cache = nodeCacheRef.current;
    const ordered: Array<{ node: StarfieldNode; role: "selected" | "upstream" | "downstream" }> = [];
    const selected = cache.get(selectedNodeId);
    if (selected) ordered.push({ node: selected, role: "selected" });
    // BFS insertion order means closest chain nodes get labels first.
    for (const id of chain.upstream) {
      const node = cache.get(id);
      if (node) ordered.push({ node, role: "upstream" });
    }
    for (const id of chain.downstream) {
      const node = cache.get(id);
      if (node) ordered.push({ node, role: "downstream" });
    }

    for (const { node, role } of ordered.slice(0, MAX_PROJECTED_LABELS)) {
      const el = document.createElement("div");
      el.style.position = "absolute";
      el.style.pointerEvents = "none";
      el.style.transform = "translate(-50%, -100%)";
      el.style.marginTop = "-14px";
      el.style.display = "none";
      el.style.zIndex = role === "selected" ? "100" : "90";
      el.style.textAlign = "center";

      const accent = baseNodeColor(node, colorMode);
      const titleColor =
        role === "selected"
          ? dark
            ? "#ffffff"
            : "#111827"
          : role === "upstream"
            ? dark
              ? "#60a5fa"
              : "#2563eb"
            : dark
              ? "#f87171"
              : "#ef4444";
      el.innerHTML = `
        <div style="font-size: 10px; color: ${accent}; font-weight: 600; white-space: nowrap; text-shadow: 0 1px 3px rgba(0,0,0,${dark ? 0.8 : 0.15});">${escapeHtml(node.type)}</div>
        <div style="font-size: ${role === "selected" ? "13.5px" : "12px"}; font-weight: 700; color: ${titleColor}; white-space: nowrap; text-shadow: 0 1px 3px rgba(0,0,0,${dark ? 0.8 : 0.15});">${escapeHtml(node.label)}</div>
      `;
      container.appendChild(el);
      labelEntriesRef.current.push({ node, el });
    }
  }

  return (
    <div data-testid="starfield-canvas" className="relative h-full w-full overflow-hidden">
      <div ref={containerRef} className="absolute inset-0" />
      <div ref={labelsRef} className="pointer-events-none absolute inset-0 z-[5] overflow-hidden" />
    </div>
  );
}

/**
 * Smooth camera focus on the node's radial direction while keeping the
 * current viewing distance and the origin as the orbit pivot. Port of
 * os-taxonomy selectNode camera logic.
 */
function focusCameraOnNode(graph: StarfieldGraph, node: StarfieldNode) {
  const cam = graph.cameraPosition();
  const distance = Math.hypot(cam.x, cam.y, cam.z) || 1300;
  const nodeDistance = Math.hypot(node.x ?? 0, node.y ?? 0, node.z ?? 0) || 1e-6;
  const ratio = distance / nodeDistance;
  graph.cameraPosition(
    { x: (node.x ?? 0) * ratio, y: (node.y ?? 0) * ratio, z: (node.z ?? 0) * ratio },
    { x: 0, y: 0, z: 0 },
    1500,
  );
}

/** Project chain-node labels from 3D world space to 2D screen coordinates. */
function projectLabels(
  graph: StarfieldGraph,
  entries: Array<{ node: StarfieldNode; el: HTMLDivElement }>,
) {
  if (entries.length === 0) return;
  const camera = graph.camera();
  const renderer = graph.renderer();
  if (!camera || !renderer) return;

  const widthHalf = renderer.domElement.clientWidth / 2;
  const heightHalf = renderer.domElement.clientHeight / 2;
  const vector = new Vector3();

  for (const { node, el } of entries) {
    vector.set(node.x ?? 0, node.y ?? 0, node.z ?? 0);
    vector.project(camera);
    if (vector.z > 1) {
      el.style.display = "none";
      continue;
    }
    el.style.display = "block";
    el.style.left = `${vector.x * widthHalf + widthHalf}px`;
    el.style.top = `${-vector.y * heightHalf + heightHalf}px`;
  }
}
