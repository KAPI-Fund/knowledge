import { useEffect } from "react";
import { useRegisterEvents, useSigma } from "@react-sigma/core";
import type { HoverState } from "./graph-render-settings";

interface EventHandlerProps {
  onNodeClick: (nodeId: string) => void;
  onHoverChange: (state: HoverState) => void;
}

export function EventHandler({ onNodeClick, onHoverChange }: EventHandlerProps) {
  const registerEvents = useRegisterEvents();
  const sigma = useSigma();

  useEffect(() => {
    registerEvents({
      clickNode: ({ node }) => onNodeClick(node),
      enterNode: ({ node }) => {
        const container = sigma.getContainer();
        container.style.cursor = "pointer";
        const graph = sigma.getGraph();
        onHoverChange({ node, neighbors: new Set(graph.neighbors(node)) });
      },
      leaveNode: () => {
        const container = sigma.getContainer();
        container.style.cursor = "default";
        onHoverChange(null);
      },
    });
  }, [registerEvents, sigma, onNodeClick, onHoverChange]);

  return null;
}
