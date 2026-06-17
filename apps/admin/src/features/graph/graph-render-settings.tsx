import { useEffect } from "react";
import { useSetSettings, useSigma } from "@react-sigma/core";
import { BASE_NODE_SIZE, GRAPH_PALETTE, mixColor } from "./graph-colors";
import { labelDensity, labelSizeThreshold } from "./graph-layout";

export type HoverState = { node: string; neighbors: Set<string> } | null;

interface GraphRenderSettingsProps {
  hoverState: HoverState;
  highlightedNodes: Set<string>;
  nodeCount: number;
}

export function GraphRenderSettings({ hoverState, highlightedNodes, nodeCount }: GraphRenderSettingsProps) {
  const sigma = useSigma();
  const setSettings = useSetSettings();

  useEffect(() => {
    setSettings({
      hideEdgesOnMove: true,
      hideLabelsOnMove: true,
      labelDensity: labelDensity(nodeCount),
      labelRenderedSizeThreshold: labelSizeThreshold(nodeCount),
      renderEdgeLabels: false,
      nodeReducer: (node, attrs) => {
        const result = { ...attrs };
        const hasHover = !!hoverState;
        const hasHighlight = highlightedNodes.size > 0;
        const isHoverNode = hoverState?.node === node;
        const isHoverNeighbor = hoverState?.neighbors.has(node) ?? false;
        const isHighlighted = highlightedNodes.has(node);

        if (isHighlighted) {
          result.size = (attrs.size ?? BASE_NODE_SIZE) * 1.5;
          result.zIndex = 10;
          result.forceLabel = true;
        }
        if (isHoverNode) {
          result.size = (attrs.size ?? BASE_NODE_SIZE) * 1.4;
          result.zIndex = 10;
          result.forceLabel = true;
        }
        if ((hasHover && !isHoverNode && !isHoverNeighbor) || (hasHighlight && !isHighlighted)) {
          result.color = mixColor(attrs.color ?? "#94a3b8", GRAPH_PALETTE.mutedNodeMixTarget, 0.75);
          result.label = "";
          result.size = (attrs.size ?? BASE_NODE_SIZE) * 0.6;
        }
        return result;
      },
      edgeReducer: (_edge, attrs) => {
        const result = { ...attrs };
        const source = String(attrs.sourceNode ?? "");
        const target = String(attrs.targetNode ?? "");
        const hasHover = !!hoverState;
        const hasHighlight = highlightedNodes.size > 0;
        const hoverEdge = hasHover && (source === hoverState?.node || target === hoverState?.node);
        const highlightedEdge =
          hasHighlight && highlightedNodes.has(source) && highlightedNodes.has(target);

        if (attrs.lowPriority && !hoverEdge && !highlightedEdge) {
          result.hidden = true;
          return result;
        }
        if ((hasHover && !hoverEdge) || (hasHighlight && !highlightedEdge)) {
          result.color = GRAPH_PALETTE.dimmedEdge;
          result.size = 0.3;
        }
        if (hoverEdge || highlightedEdge) {
          result.color = GRAPH_PALETTE.activeEdge;
          result.size = Math.max(2, (attrs.size ?? 1) * 1.5);
        }
        return result;
      },
    });
    sigma.refresh();
  }, [setSettings, sigma, hoverState, highlightedNodes, nodeCount]);

  return null;
}
