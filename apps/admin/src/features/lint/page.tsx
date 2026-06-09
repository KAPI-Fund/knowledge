import { useState } from "react";
import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";
import { ProjectFileLink } from "../shared/file-links";

import { useCreateLintTaskMutation, useLintTaskDetailQuery } from "./queries";

type LintIssue = {
  issueType: string;
  severity: string;
  page: string;
  detail: string;
  affectedPages?: string[];
};

export function LintPage() {
  const { projectId = "" } = useParams();
  const createLintTask = useCreateLintTaskMutation();
  const [activeTaskId, setActiveTaskId] = useState("");
  const task = useLintTaskDetailQuery(projectId, activeTaskId);
  const result = task.data?.result as
    | {
        mode?: string;
        issues?: LintIssue[];
      }
    | undefined
    | null;

  async function handleRunStructuralLint() {
    const created = await createLintTask.mutateAsync({
      projectId,
      mode: "structural",
    });
    setActiveTaskId(created.taskId);
  }

  async function handleRunSemanticLint() {
    const created = await createLintTask.mutateAsync({
      projectId,
      mode: "semantic",
    });
    setActiveTaskId(created.taskId);
  }

  return (
    <section className="stack">
      <h1>Lint</h1>
      <ProjectNav projectId={projectId} />
      <div className="card stack compact panel">
        <p>Run llm_wiki-style lint tasks against the current wiki.</p>
        <button type="button" onClick={handleRunStructuralLint}>
          Run Structural Lint
        </button>
        <button type="button" onClick={handleRunSemanticLint}>
          Run Semantic Lint
        </button>
      </div>

      {task.data ? (
        <section className="card stack compact panel">
          <h2>Task</h2>
          <span>{task.data.status}</span>
          {result?.mode ? <span>{result.mode}</span> : null}
        </section>
      ) : null}

      {result?.issues?.length ? (
        <ul className="results-list">
          {result.issues.map((issue, index) => (
            <li key={`${issue.page}:${issue.issueType}:${index}`} className="card stack compact panel">
              <strong>{issue.issueType}</strong>
              <span>{issue.severity}</span>
              {issue.issueType === "semantic" ? <span>{issue.page}</span> : null}
              {issue.issueType !== "semantic" ? (
                <ProjectFileLink projectId={projectId} path={`wiki/${issue.page}`} />
              ) : null}
              <span>{issue.detail}</span>
              {issue.affectedPages?.length ? (
                <div className="stack compact">
                  <strong>Affected Pages</strong>
                  <ul>
                    {issue.affectedPages.map((page) => (
                      <li key={`${issue.page}:${page}`}>
                        <ProjectFileLink projectId={projectId} path={`wiki/${page}`} />
                      </li>
                    ))}
                  </ul>
                </div>
              ) : null}
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}
