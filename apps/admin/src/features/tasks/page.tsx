import { useParams } from "react-router-dom";
import { useState } from "react";

import { ProjectNav } from "../projects/project-nav";
import { ProjectFileLink } from "../shared/file-links";
import { useCancelTaskMutation, useProjectTasksQuery, useRetryTaskMutation, useTaskDetailQuery } from "./queries";

export function TasksPage() {
  const { projectId = "" } = useParams();
  const tasks = useProjectTasksQuery(projectId);
  const retryTask = useRetryTaskMutation();
  const cancelTask = useCancelTaskMutation();
  const [selectedTaskId, setSelectedTaskId] = useState("");
  const taskId = selectedTaskId || tasks.data?.[0]?.id || "";
  const detail = useTaskDetailQuery(projectId, taskId);

  return (
    <section className="stack">
      <h1>Tasks</h1>
      <ProjectNav projectId={projectId} />
      <ul className="results-list">
        {tasks.data?.map((task) => (
          <li key={task.id} className="card stack compact panel">
            <strong>{task.title}</strong> <span>{task.status}</span>
            <span>{task.taskType}</span>
            {task.relativePath ? (
              <ProjectFileLink projectId={projectId} path={task.relativePath} />
            ) : null}
            <span>
              Attempts {(task.attemptCount ?? 0).toString()}/{(task.maxAttempts ?? 0).toString()}
            </span>
            <button type="button" onClick={() => setSelectedTaskId(task.id)}>
              Inspect
            </button>
            <button
              type="button"
              onClick={() => retryTask.mutateAsync({ projectId, taskId: task.id })}
            >
              Retry
            </button>
            {!["completed", "succeeded", "failed", "cancelled"].includes(task.status) ? (
              <button
                type="button"
                onClick={() => cancelTask.mutateAsync({ projectId, taskId: task.id })}
              >
                Cancel
              </button>
            ) : null}
          </li>
        ))}
      </ul>
      {detail.data ? (
        <section className="card stack compact panel">
          <h2>Task Detail</h2>
          <span>{detail.data.title}</span>
          <span>{detail.data.taskType}</span>
          <span>{detail.data.status}</span>
          {detail.data.relativePath ? (
            <ProjectFileLink projectId={projectId} path={detail.data.relativePath} />
          ) : null}
          {detail.data.error ? <pre>{JSON.stringify(detail.data.error, null, 2)}</pre> : null}
          {detail.data.result ? <pre>{JSON.stringify(detail.data.result, null, 2)}</pre> : null}
          <pre>{JSON.stringify(detail.data.detail, null, 2)}</pre>
        </section>
      ) : null}
    </section>
  );
}
