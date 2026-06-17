import Graph from "graphology";
import forceAtlas2 from "graphology-layout-forceatlas2";

interface LayoutMessage {
  key: string;
  nodes: Array<{ id: string; x: number; y: number }>;
  edges: Array<{ source: string; target: string; weight: number }>;
  iterations: number;
  scalingRatio: number;
}

self.onmessage = (event: MessageEvent<LayoutMessage>) => {
  const { key, nodes, edges, iterations, scalingRatio } = event.data;

  const graph = new Graph();
  for (const node of nodes) {
    graph.addNode(node.id, { x: node.x, y: node.y });
  }
  for (const edge of edges) {
    if (
      graph.hasNode(edge.source) &&
      graph.hasNode(edge.target) &&
      !graph.hasEdge(edge.source, edge.target)
    ) {
      graph.addEdge(edge.source, edge.target, { weight: edge.weight });
    }
  }

  const settings = forceAtlas2.inferSettings(graph);
  forceAtlas2.assign(graph, {
    iterations,
    settings: {
      ...settings,
      gravity: 1,
      scalingRatio,
      strongGravityMode: true,
      barnesHutOptimize: nodes.length > 50,
    },
  });

  const positions = graph.mapNodes((id, attrs) => ({
    id,
    x: attrs.x as number,
    y: attrs.y as number,
  }));

  (self as unknown as Worker).postMessage({ key, positions });
};
