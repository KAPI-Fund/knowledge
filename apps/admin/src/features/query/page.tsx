import { useState } from "react";
import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";
import { useCancelTaskMutation, useRetryTaskMutation } from "../tasks/queries";

import {
  useCreateQueryTaskMutation,
  useQueryTaskDetailQuery,
  useSaveQueryTaskMutation,
} from "./queries";

type QueryCitation = {
  path: string;
  title: string;
  snippet: string;
  score: number;
};

export function QueryPage() {
  const { projectId = "" } = useParams();
  const createQueryTask = useCreateQueryTaskMutation();
  const retryTask = useRetryTaskMutation();
  const cancelTask = useCancelTaskMutation();
  const saveQueryTask = useSaveQueryTaskMutation();
  const [query, setQuery] = useState("");
  const [topK, setTopK] = useState("3");
  const [activeTaskId, setActiveTaskId] = useState("");
  const [saveTaskId, setSaveTaskId] = useState("");
  const task = useQueryTaskDetailQuery(projectId, activeTaskId);
  const saveTask = useQueryTaskDetailQuery(projectId, saveTaskId);

  async function handleRunQuery() {
    const trimmed = query.trim();
    if (!trimmed) {
      return;
    }

    const created = await createQueryTask.mutateAsync({
      projectId,
      query: trimmed,
      topK: Number(topK) || 3,
    });
    setActiveTaskId(created.taskId);
    setSaveTaskId("");
  }

  async function handleSaveToWiki() {
    if (!task.data) {
      return;
    }

    const title = deriveSaveTitle(query, task.data.title);
    const created = await saveQueryTask.mutateAsync({
      projectId,
      taskId: task.data.id,
      title,
    });
    setSaveTaskId(created.taskId);
  }

  const result = task.data?.result as
    | {
        answer?: string;
        citations?: QueryCitation[];
        contextSummary?: string;
      }
    | undefined
    | null;
  const error = task.data?.error as { message?: string } | undefined | null;

  return (
    <section className="stack">
      <h1>Query</h1>
      <ProjectNav projectId={projectId} />
      <div className="card stack panel">
        <label>
          Query
          <textarea
            aria-label="Query"
            rows={5}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
        <label>
          Top K
          <input value={topK} onChange={(event) => setTopK(event.target.value)} />
        </label>
        <button type="button" onClick={handleRunQuery}>
          Run Query
        </button>
      </div>

      {task.data ? (
        <section className="card stack compact panel">
          <h2>Task</h2>
          <span>{task.data.status}</span>
          {task.data.status === "failed" ? (
            <button
              type="button"
              onClick={() => retryTask.mutateAsync({ projectId, taskId: task.data.id })}
            >
              Retry
            </button>
          ) : null}
          {!["succeeded", "failed", "cancelled"].includes(task.data.status) ? (
            <button
              type="button"
              onClick={() => cancelTask.mutateAsync({ projectId, taskId: task.data.id })}
            >
              Cancel
            </button>
          ) : null}
        </section>
      ) : null}

      {result?.answer ? (
        <section className="card stack panel">
          <h2>Answer</h2>
          <p>{result.answer}</p>
          {result.contextSummary ? <p>{result.contextSummary}</p> : null}
          <button type="button" onClick={handleSaveToWiki}>
            Save To Wiki
          </button>
          {saveTask.data?.status === "succeeded" ? (
            <p>
              Saved to wiki{" "}
              {String(
                (saveTask.data.result as { relativePath?: string } | undefined | null)
                  ?.relativePath ?? "",
              )}
            </p>
          ) : null}
          {saveTask.data?.status === "failed" ? <p>Save to wiki failed</p> : null}
          <ul className="results-list">
            {(result.citations ?? []).map((citation) => (
              <li key={citation.path} className="card stack compact">
                <strong>{citation.title}</strong>
                <span>{citation.path}</span>
                <span>{citation.snippet}</span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      {error?.message ? (
        <section className="card stack compact panel">
          <h2>Error</h2>
          <p>{error.message}</p>
        </section>
      ) : null}
    </section>
  );
}

function deriveSaveTitle(query: string, taskTitle: string) {
  const trimmedQuery = query.trim();
  if (trimmedQuery) {
    return trimmedQuery;
  }

  return taskTitle.replace(/^Query:\s*/, "").trim() || "Saved Query";
}
