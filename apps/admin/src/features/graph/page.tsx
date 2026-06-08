import { useState } from "react";
import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";

import { useProjectGraphNeighborsQuery, useProjectGraphQuery } from "./queries";

export function GraphPage() {
  const { projectId = "" } = useParams();
  const [query, setQuery] = useState("");
  const [graphQuery, setGraphQuery] = useState("");
  const [nodeTypeInput, setNodeTypeInput] = useState("");
  const [graphNodeType, setGraphNodeType] = useState("");
  const [limitInput, setLimitInput] = useState("100");
  const [graphLimit, setGraphLimit] = useState(100);
  const graph = useProjectGraphQuery(projectId, graphQuery, graphNodeType, graphLimit);
  const [selectedNodeId, setSelectedNodeId] = useState("");
  const neighbors = useProjectGraphNeighborsQuery(projectId, selectedNodeId);

  function applyFilters() {
    setGraphQuery(query.trim());
    setGraphNodeType(nodeTypeInput.trim().toLowerCase());
    setGraphLimit(Number(limitInput) || 100);
  }

  return (
    <section className="stack">
      <h1>Graph</h1>
      <ProjectNav projectId={projectId} />
      <div className="card stack compact panel">
        <label>
          Graph Filter
          <input value={query} onChange={(event) => setQuery(event.target.value)} />
        </label>
        <label>
          Node Type
          <select value={nodeTypeInput} onChange={(event) => setNodeTypeInput(event.target.value)}>
            <option value="">all</option>
            <option value="concept">concept</option>
            <option value="entity">entity</option>
            <option value="source">source</option>
            <option value="query">query</option>
            <option value="other">other</option>
          </select>
        </label>
        <label>
          Node Limit
          <input value={limitInput} onChange={(event) => setLimitInput(event.target.value)} />
        </label>
        <button type="button" onClick={applyFilters}>
          Apply Graph Filters
        </button>
      </div>
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
              <span>{node.nodeType}</span>
              <span>{node.path}</span>
              <span>{`Links ${node.linkCount}`}</span>
            </li>
          ))}
        </ul>
        <section className="card stack compact panel">
          <h2>Neighbors</h2>
          {selectedNodeId ? (
            <>
              <strong>{neighbors.data?.node.label ?? selectedNodeId}</strong>
              <span>{`Links ${neighbors.data?.node.linkCount ?? 0}`}</span>
              <ul>
                {neighbors.data?.neighbors.map((node) => (
                  <li key={node.id}>{`${node.label} (${node.linkCount})`}</li>
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
