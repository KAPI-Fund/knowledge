import { useState } from "react";
import { SigmaContainer } from "@react-sigma/core";
import "@react-sigma/core/lib/style.css";
import { DEFAULT_GRAPH_SPACING, DEFAULT_NODE_SCALE } from "./graph-colors";
import { GraphLoader, type ColorMode } from "./graph-loader";
import { GraphRenderSettings, type HoverState } from "./graph-render-settings";
import { EventHandler } from "./graph-events";
import { ZoomControls } from "./zoom-controls";
import type { GraphEdge, GraphNode } from "./types";

export interface GraphCanvasProps {
  nodes: GraphNode[];
  edges: GraphEdge[];
  colorMode: ColorMode;
  highlightedNodes: Set<string>;
  onNodeClick: (nodeId: string) => void;
  nodeScale?: number;
  graphSpacing?: number;
}

const SIGMA_SETTINGS = {
  defaultNodeType: "circle",
  hideEdgesOnMove: true,
  hideLabelsOnMove: true,
  renderEdgeLabels: false,
  labelSize: 13,
  labelWeight: "bold",
  stagePadding: 30,
} as const;

export function GraphCanvas({
  nodes,
  edges,
  colorMode,
  highlightedNodes,
  onNodeClick,
  nodeScale = DEFAULT_NODE_SCALE,
  graphSpacing = DEFAULT_GRAPH_SPACING,
}: GraphCanvasProps) {
  const [hoverState, setHoverState] = useState<HoverState>(null);

  return (
    <div data-testid="graph-canvas" className="relative h-full w-full">
      <SigmaContainer style={{ height: "100%", width: "100%" }} settings={SIGMA_SETTINGS}>
        <GraphLoader
          nodes={nodes}
          edges={edges}
          colorMode={colorMode}
          nodeScale={nodeScale}
          graphSpacing={graphSpacing}
        />
        <GraphRenderSettings
          hoverState={hoverState}
          highlightedNodes={highlightedNodes}
          nodeCount={nodes.length}
        />
        <EventHandler onNodeClick={onNodeClick} onHoverChange={setHoverState} />
        <ZoomControls />
      </SigmaContainer>
    </div>
  );
}
