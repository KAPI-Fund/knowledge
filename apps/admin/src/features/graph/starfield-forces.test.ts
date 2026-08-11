import { describe, expect, it, vi } from "vitest";

import {
  applyLayoutForces,
  applyLayoutNodeStates,
  cylinderForce,
  radialForce,
  ringForce,
  RING_RADIUS,
  SPHERE_RADIUS,
  TORNADO_LAYER_HEIGHT,
  type StarfieldNode,
} from "./starfield-forces";

function node(partial: Partial<StarfieldNode>): StarfieldNode {
  return {
    id: "n",
    label: "N",
    type: "concept",
    path: "wiki/n.md",
    linkCount: 0,
    community: 0,
    sources: [],
    vx: 0,
    vy: 0,
    vz: 0,
    ...partial,
  };
}

describe("radialForce", () => {
  it("pushes inner nodes outward and outer nodes inward toward the shell", () => {
    const inner = node({ x: 10, y: 0, z: 0 });
    const outer = node({ x: SPHERE_RADIUS * 2, y: 0, z: 0 });
    const force = radialForce(SPHERE_RADIUS);
    force.initialize([inner, outer]);
    force(1);
    expect(inner.vx).toBeGreaterThan(0);
    expect(outer.vx).toBeLessThan(0);
  });
});

describe("ringForce", () => {
  it("initializes torus coordinates once and pulls nodes toward them", () => {
    const n = node({ x: 0, y: 0, z: 0 });
    const force = ringForce();
    force.initialize([n]);
    force(1);
    expect(n.ringX).toBeDefined();
    expect(n.ringY).toBeDefined();
    expect(n.ringZ).toBeDefined();
    // torus center radius is RING_RADIUS, so the ring coordinate is far from origin
    const ringDistance = Math.hypot(n.ringX ?? 0, n.ringZ ?? 0);
    expect(ringDistance).toBeGreaterThan(RING_RADIUS / 2);
    // velocity points from origin toward the ring coordinate
    expect(Math.sign(n.vx ?? 0)).toBe(Math.sign(n.ringX ?? 0));
    const firstRingX = n.ringX;
    force(1);
    expect(n.ringX).toBe(firstRingX);
  });
});

describe("cylinderForce", () => {
  it("pulls nodes toward the layer-dependent cone radius on the XZ plane", () => {
    const shallow = node({ x: 500, z: 0, layer: 0 }); // target radius 15 -> pull inward
    const deep = node({ x: 1, z: 0, layer: 5 }); // target radius 290 -> push outward
    const force = cylinderForce();
    force.initialize([shallow, deep]);
    force(1);
    expect(shallow.vx).toBeLessThan(0);
    expect(deep.vx).toBeGreaterThan(0);
    expect(shallow.vy).toBe(0);
  });
});

describe("applyLayoutNodeStates", () => {
  it("pins fy by layer for tornado and frees all axes otherwise", () => {
    const a = node({ layer: 0 });
    const b = node({ layer: 4 });
    applyLayoutNodeStates([a, b], "tornado", 4);
    expect(a.fy).toBe(-2 * TORNADO_LAYER_HEIGHT);
    expect(b.fy).toBe(2 * TORNADO_LAYER_HEIGHT);
    applyLayoutNodeStates([a, b], "sphere", 4);
    expect(a.fy).toBeUndefined();
    expect(b.fy).toBeUndefined();
  });
});

describe("applyLayoutForces", () => {
  function fakeGraph() {
    const accessor = { strength: vi.fn(), distance: vi.fn() };
    accessor.strength.mockReturnValue(accessor);
    accessor.distance.mockReturnValue(accessor);
    const setForces = new Map<string, unknown>();
    const graph = {
      d3Force: vi.fn((name: string, force?: unknown) => {
        if (force !== undefined) {
          setForces.set(name, force);
          return graph;
        }
        return accessor;
      }),
    };
    return { graph, accessor, setForces };
  }

  it("installs exactly one shape force per layout and clears the others", () => {
    const { graph, setForces } = fakeGraph();
    applyLayoutForces(graph, "sphere");
    expect(setForces.get("radial")).toBeTypeOf("function");
    expect(setForces.get("ring")).toBeNull();
    expect(setForces.get("cylinder")).toBeNull();

    applyLayoutForces(graph, "ring");
    expect(setForces.get("ring")).toBeTypeOf("function");
    expect(setForces.get("radial")).toBeNull();

    applyLayoutForces(graph, "tornado");
    expect(setForces.get("cylinder")).toBeTypeOf("function");
    expect(setForces.get("ring")).toBeNull();
  });

  it("tunes charge and link forces per layout", () => {
    const { graph, accessor } = fakeGraph();
    applyLayoutForces(graph, "sphere");
    expect(accessor.strength).toHaveBeenCalledWith(-60);
    expect(accessor.distance).toHaveBeenCalledWith(50);
    expect(accessor.strength).toHaveBeenCalledWith(0.08);
  });
});
