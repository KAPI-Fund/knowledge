import { useState } from "react";
import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";
import { ProjectFileLink } from "../shared/file-links";

import { useProjectSearchMutation } from "./queries";

export function SearchPage() {
  const { projectId = "" } = useParams();
  const [query, setQuery] = useState("");
  const [topK, setTopK] = useState("10");
  const [includeContent, setIncludeContent] = useState(false);
  const [response, setResponse] = useState<{
    mode: string;
    tokenHits: number;
    vectorHits: number;
    results: Array<{
      path: string;
      title: string;
      snippet: string;
      score: number;
      images?: Array<{
        url: string;
        alt: string;
      }>;
      content?: string;
    }>;
  } | null>(null);
  const search = useProjectSearchMutation();

  async function handleSearch() {
    const trimmed = query.trim();
    if (!trimmed) {
      setResponse(null);
      return;
    }

    const next = await search.mutateAsync({
      projectId,
      query: trimmed,
      topK: Number(topK) || 10,
      includeContent,
    });
    setResponse(next);
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
        <label>
          Top K
          <input value={topK} onChange={(event) => setTopK(event.target.value)} />
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={includeContent}
            onChange={(event) => setIncludeContent(event.target.checked)}
          />
          Include Content
        </label>
        <button type="button" onClick={handleSearch}>
          Run Search
        </button>
      </div>
      {response ? (
        <div className="stats">
          <span>{`Mode: ${response.mode}`}</span>
          <span>{`Token Hits: ${response.tokenHits}`}</span>
          <span>{`Vector Hits: ${response.vectorHits}`}</span>
        </div>
      ) : null}
      <ul className="results-list">
        {response?.results.map((result) => (
          <li key={result.path} className="card stack compact panel">
            <strong>{result.title}</strong>
            <ProjectFileLink projectId={projectId} path={result.path} />
            <span>{result.snippet}</span>
            {result.images?.length ? (
              <div className="stack compact">
                <strong>Images</strong>
                <ul>
                  {result.images.map((image) => (
                    <li key={`${result.path}:${image.url}`}>
                      <span>{image.alt}</span> <code>{image.url}</code>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
            {result.content ? <pre className="preview-pane">{result.content}</pre> : null}
          </li>
        ))}
      </ul>
    </section>
  );
}
