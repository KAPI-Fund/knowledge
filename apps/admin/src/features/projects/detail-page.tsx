import { useParams } from "react-router-dom";

import { useProjectAuditLogsQuery } from "../audit/queries";
import { useProjectReviewsQuery } from "../reviews/queries";
import { useProjectSourceWatchQuery } from "../source-watch/queries";
import { useProjectSourcesQuery } from "../sources/queries";
import { useProjectTasksQuery } from "../tasks/queries";
import { useProjectDetailQuery } from "./detail-queries";
import { ProjectNav } from "./project-nav";

export function ProjectDetailPage() {
  const { projectId = "" } = useParams();
  const project = useProjectDetailQuery(projectId);
  const sources = useProjectSourcesQuery(projectId);
  const tasks = useProjectTasksQuery(projectId);
  const reviews = useProjectReviewsQuery(projectId, {
    status: "unresolved",
    itemType: "",
    limit: 5,
  });
  const auditLogs = useProjectAuditLogsQuery(projectId);
  const sourceWatch = useProjectSourceWatchQuery(projectId);
  const detail = project.data?.project;

  if (!detail) {
    return <section><h1>Project</h1></section>;
  }

  const recentSources = (sources.data ?? []).slice(0, 5);
  const recentTasks = (tasks.data ?? []).slice(0, 5);
  const recentReviews = (reviews.data ?? []).slice(0, 5);
  const recentAudit = (auditLogs.data ?? []).slice(0, 5);

  return (
    <section className="stack">
      <header className="stack compact">
        <h1>{detail.name}</h1>
        <p>{detail.rootPath}</p>
      </header>
      <div className="stats">
        <span>{detail.sourceCount} sources</span>
        <span>{detail.taskCount} tasks</span>
        <span>{detail.reviewCount} reviews</span>
      </div>
      <ProjectNav projectId={detail.id} />
      <section className="card stack compact panel">
        <h2>Project Health</h2>
        {sourceWatch.data ? (
          <p>{sourceWatch.data.enabled ? "Source watch enabled" : "Source watch disabled"}</p>
        ) : null}
      </section>
      <section className="card stack compact panel">
        <h2>Recent Sources</h2>
        <ul className="results-list">
          {recentSources.map((source) => (
            <li key={source.relativePath}>
              <strong>{source.relativePath}</strong>
              <span>{source.size}</span>
            </li>
          ))}
        </ul>
      </section>
      <section className="card stack compact panel">
        <h2>Recent Tasks</h2>
        <ul className="results-list">
          {recentTasks.map((task) => (
            <li key={task.id}>
              <strong>{task.title}</strong>
              <span>{task.status}</span>
            </li>
          ))}
        </ul>
      </section>
      <section className="card stack compact panel">
        <h2>Recent Reviews</h2>
        <ul className="results-list">
          {recentReviews.map((review) => (
            <li key={review.id}>
              <strong>{review.title}</strong>
              <span>{review.status}</span>
            </li>
          ))}
        </ul>
      </section>
      <section className="card stack compact panel">
        <h2>Recent Audit</h2>
        <ul className="results-list">
          {recentAudit.map((item) => (
            <li key={item.id}>
              <strong>{item.action}</strong>
              <span>{item.summary}</span>
            </li>
          ))}
        </ul>
      </section>
    </section>
  );
}
