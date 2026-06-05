import { useState } from "react";
import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";
import {
  useDeleteSourceMutation,
  useImportSourceMutation,
  useIngestSourceMutation,
  useProjectSourcesQuery,
  useRescanSourcesMutation,
} from "./queries";

export function SourcesPage() {
  const { projectId = "" } = useParams();
  const sources = useProjectSourcesQuery(projectId);
  const importSource = useImportSourceMutation();
  const ingestSource = useIngestSourceMutation();
  const rescanSources = useRescanSourcesMutation();
  const deleteSource = useDeleteSourceMutation();
  const [fileName, setFileName] = useState("");
  const [content, setContent] = useState("");

  async function handleImportSource() {
    const trimmedFileName = fileName.trim();
    if (!trimmedFileName) {
      return;
    }

    await importSource.mutateAsync({
      projectId,
      fileName: trimmedFileName,
      content,
    });
    setFileName("");
    setContent("");
  }

  return (
    <section className="stack">
      <h1>Sources</h1>
      <ProjectNav projectId={projectId} />
      <div className="card stack panel">
        <label>
          File Name
          <input value={fileName} onChange={(event) => setFileName(event.target.value)} />
        </label>
        <label>
          Markdown Content
          <textarea value={content} onChange={(event) => setContent(event.target.value)} rows={6} />
        </label>
        <button type="button" onClick={handleImportSource}>
          Import Source
        </button>
        <button type="button" onClick={() => rescanSources.mutateAsync({ projectId })}>
          Rescan Sources
        </button>
      </div>
      <ul>
        {sources.data?.map((source) => (
          <li key={source.relativePath}>
            <span>{source.relativePath}</span> <span>{source.size}</span>
            <button
              type="button"
              onClick={() =>
                ingestSource.mutateAsync({
                  projectId,
                  relativePath: source.relativePath,
                })
              }
            >
              Ingest
            </button>
            <button
              type="button"
              onClick={() =>
                deleteSource.mutateAsync({
                  projectId,
                  relativePath: source.relativePath.replace(/^raw\/sources\//, ""),
                })
              }
            >
              Delete
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
