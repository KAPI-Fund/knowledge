import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";
import { useProjectAuditLogsQuery } from "./queries";

export function AuditPage() {
  const { projectId = "" } = useParams();
  const items = useProjectAuditLogsQuery(projectId);

  return (
    <section className="stack">
      <h1>Audit</h1>
      <ProjectNav projectId={projectId} />
      <ul>
        {items.data?.map((item) => (
          <li key={item.id}>
            <strong>{item.action}</strong> <span>{item.summary}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
