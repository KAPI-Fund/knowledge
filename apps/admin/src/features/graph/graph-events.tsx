import { useEffect } from "react";
import { useRegisterEvents, useSigma } from "@react-sigma/core";
import type { SigmaNodeEventPayload } from "sigma/types";
import type { HoverState } from "./graph-render-settings";

interface EventHandlerProps {
  onNodeClick: (nodeId: string) => void;
  onNodeContextMenu: (nodeId: string, x: number, y: number) => void;
  onHoverChange: (state: HoverState) => void;
}

/** Port of upstream graph-view.tsx:487-490. */
function clientPointFromEvent(event: MouseEvent | TouchEvent): { x: number; y: number } {
  if ("clientX" in event) return { x: event.clientX, y: event.clientY };
  const touch = event.touches[0] ?? event.changedTouches[0];
  return touch ? { x: touch.clientX, y: touch.clientY } : { x: 0, y: 0 };
}

export function EventHandler({ onNodeClick, onNodeContextMenu, onHoverChange }: EventHandlerProps) {
  const registerEvents = useRegisterEvents();
  const sigma = useSigma();

  useEffect(() => {
    registerEvents({
      clickNode: ({ node }) => onNodeClick(node),
      rightClickNode: (payload: SigmaNodeEventPayload) => {
        payload.preventSigmaDefault();
        payload.event.original.preventDefault();
        const point = clientPointFromEvent(payload.event.original);
        onNodeContextMenu(payload.node, point.x, point.y);
      },
      rightClickStage: () => onNodeContextMenu("", 0, 0),
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
  }, [registerEvents, sigma, onNodeClick, onNodeContextMenu, onHoverChange]);

  return null;
}
