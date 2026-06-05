import { useState } from "react";
import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";

import { useProjectSearchMutation } from "./queries";

export function SearchPage() {
  const { projectId = "" } = useParams();
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<
    Array<{ path: string; title: string; snippet: string; score: number }>
  >([]);
  const search = useProjectSearchMutation();

  async function handleSearch() {
    const trimmed = query.trim();
    if (!trimmed) {
      setResults([]);
      return;
    }

    const next = await search.mutateAsync({ projectId, query: trimmed });
    setResults(next);
  }

  return (
    <section className="stack">
      <h1>Search</h1>
      <ProjectNav projectId={projectId} />
      <div className="card stack compact panel">
        <label>
          Search Query
          <input value={query} onChange={(event) => setQuery(event.target.value)} />
        </label>
        <button type="button" onClick={handleSearch}>
          Run Search
        </button>
      </div>
      <ul className="results-list">
        {results.map((result) => (
          <li key={result.path} className="card stack compact panel">
            <strong>{result.title}</strong>
            <span>{result.path}</span>
            <span>{result.snippet}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
