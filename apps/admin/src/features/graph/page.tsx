import { useState } from "react";
import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";

import { useProjectGraphNeighborsQuery, useProjectGraphQuery } from "./queries";

export function GraphPage() {
  const { projectId = "" } = useParams();
  const graph = useProjectGraphQuery(projectId);
  const [selectedNodeId, setSelectedNodeId] = useState("");
  const neighbors = useProjectGraphNeighborsQuery(projectId, selectedNodeId);

  return (
    <section className="stack">
      <h1>Graph</h1>
      <ProjectNav projectId={projectId} />
      <div className="stats">
        <span>Nodes: {graph.data?.nodes.length ?? 0}</span>
        <span>Edges: {graph.data?.edges.length ?? 0}</span>
      </div>
      <div className="graph-layout">
        <ul className="results-list">
          {graph.data?.nodes.map((node) => (
            <li key={node.id} className="card stack compact panel">
              <button
                type="button"
                className="ghost-button"
                onClick={() => setSelectedNodeId(node.id)}
              >
                {node.label}
              </button>
              <span>{node.id}</span>
              <span>{node.path}</span>
            </li>
          ))}
        </ul>
        <section className="card stack compact panel">
          <h2>Neighbors</h2>
          {selectedNodeId ? (
            <>
              <strong>{neighbors.data?.node.label ?? selectedNodeId}</strong>
              <ul>
                {neighbors.data?.neighbors.map((node) => (
                  <li key={node.id}>{node.label}</li>
                ))}
              </ul>
            </>
          ) : (
            <p>Select a node to inspect its neighborhood.</p>
          )}
        </section>
      </div>
    </section>
  );
}
