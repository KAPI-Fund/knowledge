import { useParams } from "react-router-dom";

import { useProjectDetailQuery } from "./detail-queries";
import { ProjectNav } from "./project-nav";

export function ProjectDetailPage() {
  const { projectId = "" } = useParams();
  const project = useProjectDetailQuery(projectId);
  const detail = project.data?.project;

  if (!detail) {
    return <section><h1>Project</h1></section>;
  }

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
    </section>
  );
}
