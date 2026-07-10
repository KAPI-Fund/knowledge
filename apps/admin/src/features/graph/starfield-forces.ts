import type { GraphNode } from "./types";

export type StarfieldLayout = "sphere" | "ring" | "tornado";

/** Node shape after 3d-force-graph / d3-force-3d mutates it with simulation state. */
export interface StarfieldNode extends GraphNode {
  x?: number;
  y?: number;
  z?: number;
  vx?: number;
  vy?: number;
  vz?: number;
  fx?: number;
  fy?: number;
  fz?: number;
  ringX?: number;
  ringY?: number;
  ringZ?: number;
  layer?: number;
}

interface CustomForce {
  (alpha: number): void;
  initialize: (nodes: StarfieldNode[]) => void;
}

export const SPHERE_RADIUS = 350;
export const RING_RADIUS = 320;
export const RING_TUBE_RADIUS = 70;
export const TORNADO_LAYER_HEIGHT = 130;

/** Spherical shell pull force. Port of os-taxonomy index.html radialForce. */
export function radialForce(radius: number): CustomForce {
  let nodes: StarfieldNode[] = [];
  const force = ((alpha: number) => {
    for (const node of nodes) {
      const dx = node.x || 1e-6;
      const dy = node.y || 1e-6;
      const dz = node.z || 1e-6;
      const r = Math.hypot(dx, dy, dz) || 1e-6;
      const factor = ((radius - r) / r) * alpha * 0.5;
      node.vx = (node.vx ?? 0) + dx * factor;
      node.vy = (node.vy ?? 0) + dy * factor;
      node.vz = (node.vz ?? 0) + dz * factor;
    }
  }) as CustomForce;
  force.initialize = (n) => {
    nodes = n;
  };
  return force;
}

/** Torus ("swim ring") pull force. Port of os-taxonomy index.html ringForce. */
export function ringForce(): CustomForce {
  let nodes: StarfieldNode[] = [];
  const force = ((alpha: number) => {
    for (const node of nodes) {
      if (node.ringX === undefined) {
        const theta = Math.random() * Math.PI * 2;
        const phi = Math.random() * Math.PI * 2;
        node.ringX = (RING_RADIUS + RING_TUBE_RADIUS * Math.cos(phi)) * Math.cos(theta);
        node.ringZ = (RING_RADIUS + RING_TUBE_RADIUS * Math.cos(phi)) * Math.sin(theta);
        node.ringY = RING_TUBE_RADIUS * Math.sin(phi);
      }
      const dx = node.ringX - (node.x ?? 0);
      const dy = (node.ringY ?? 0) - (node.y ?? 0);
      const dz = (node.ringZ ?? 0) - (node.z ?? 0);
      // Strong 0.45 lock prevents nodes from collapsing toward the center.
      node.vx = (node.vx ?? 0) + dx * alpha * 0.45;
      node.vy = (node.vy ?? 0) + dy * alpha * 0.45;
      node.vz = (node.vz ?? 0) + dz * alpha * 0.45;
    }
  }) as CustomForce;
  force.initialize = (n) => {
    nodes = n;
  };
  return force;
}

/**
 * Tornado cone force on the XZ plane. Port of os-taxonomy index.html
 * cylinderForce, with the original age-based tier replaced by BFS dependency
 * depth (node.layer).
 */
export function cylinderForce(): CustomForce {
  let nodes: StarfieldNode[] = [];
  const force = ((alpha: number) => {
    for (const node of nodes) {
      const targetR = (node.layer ?? 0) * 55 + 15;
      const dx = node.x ?? 1e-6;
      const dz = node.z ?? 1e-6;
      const r = Math.hypot(dx, dz) || 1e-6;
      const factor = ((targetR - r) / r) * alpha * 0.4;
      node.vx = (node.vx ?? 0) + dx * factor;
      node.vz = (node.vz ?? 0) + dz * factor;
    }
  }) as CustomForce;
  force.initialize = (n) => {
    nodes = n;
  };
  return force;
}

/**
 * Tornado pins each node's height to its dependency layer; the other layouts
 * leave the physics engine fully free. Port of applyLayoutNodeStates.
 */
export function applyLayoutNodeStates(
  nodes: StarfieldNode[],
  layout: StarfieldLayout,
  maxLayer: number,
): void {
  for (const node of nodes) {
    if (layout === "tornado") {
      node.fy = ((node.layer ?? 0) - maxLayer / 2) * TORNADO_LAYER_HEIGHT;
      node.fx = undefined;
      node.fz = undefined;
    } else {
      node.fx = undefined;
      node.fy = undefined;
      node.fz = undefined;
    }
  }
}

interface D3ForceAccessor {
  strength: (value: number) => D3ForceAccessor;
  distance: (value: number) => D3ForceAccessor;
}

export interface ForceGraphForces {
  d3Force(name: string): unknown;
  d3Force(name: string, force: CustomForce | null): unknown;
}

/** Per-layout physics tuning. Port of os-taxonomy applyLayoutForces. */
export function applyLayoutForces(graph: ForceGraphForces, layout: StarfieldLayout): void {
  const charge = graph.d3Force("charge") as D3ForceAccessor;
  const link = graph.d3Force("link") as D3ForceAccessor;

  if (layout === "tornado") {
    graph.d3Force("cylinder", cylinderForce());
    graph.d3Force("radial", null);
    graph.d3Force("ring", null);
    charge.strength(-40);
    link.distance(40).strength(0.3);
  } else if (layout === "sphere") {
    graph.d3Force("cylinder", null);
    graph.d3Force("radial", radialForce(SPHERE_RADIUS));
    graph.d3Force("ring", null);
    // Stronger repulsion + weak link pull keeps the shell evenly spread.
    charge.strength(-60);
    link.distance(50).strength(0.08);
  } else {
    graph.d3Force("cylinder", null);
    graph.d3Force("radial", null);
    graph.d3Force("ring", ringForce());
    // High repulsion clears the ring's center; near-zero link pull avoids collapse.
    charge.strength(-70);
    link.distance(75).strength(0.015);
  }
}
