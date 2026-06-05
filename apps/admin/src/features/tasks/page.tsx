import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";
import { useCancelTaskMutation, useProjectTasksQuery, useRetryTaskMutation, useTaskDetailQuery } from "./queries";

export function TasksPage() {
  const { projectId = "" } = useParams();
  const tasks = useProjectTasksQuery(projectId);
  const retryTask = useRetryTaskMutation();
  const cancelTask = useCancelTaskMutation();
  const taskId = tasks.data?.[0]?.id ?? "";
  const detail = useTaskDetailQuery(projectId, taskId);

  return (
    <section className="stack">
      <h1>Tasks</h1>
      <ProjectNav projectId={projectId} />
      <ul>
        {tasks.data?.map((task) => (
          <li key={task.id}>
            <strong>{task.title}</strong> <span>{task.status}</span>
            {task.relativePath ? <span>{task.relativePath}</span> : null}
            <button
              type="button"
              onClick={() => retryTask.mutateAsync({ projectId, taskId: task.id })}
            >
              Retry
            </button>
            {task.status !== "completed" ? (
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
          {detail.data.relativePath ? <span>{detail.data.relativePath}</span> : null}
          <pre>{JSON.stringify(detail.data.detail, null, 2)}</pre>
        </section>
      ) : null}
    </section>
  );
}
