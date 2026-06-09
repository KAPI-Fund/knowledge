import { Link } from "react-router-dom";

import { useSystemSettingsQuery } from "../settings/queries";
import { useProjectsQuery } from "../projects/queries";

export function DashboardPage() {
  const projects = useProjectsQuery();
  const settings = useSystemSettingsQuery();
  const projectList = projects.data ?? [];
  const recentProjects = projectList.slice(0, 5);

  return (
    <section className="stack">
      <header className="stack compact">
        <h1>Dashboard</h1>
        <p>Workspace overview</p>
      </header>
      <div className="stats">
        <span>{projectList.length} projects</span>
        <span>{settings.data?.language ?? "unknown"}</span>
        <span>{settings.data?.defaultQueryLimit ?? 0} query limit</span>
      </div>
      <section className="card stack compact panel">
        <h2>Recent Projects</h2>
        <ul className="results-list">
          {recentProjects.map((project) => (
            <li key={project.id}>
              <strong>
                <Link to={`/projects/${project.id}`}>{project.name}</Link>
              </strong>
              <span>{project.rootPath}</span>
            </li>
          ))}
        </ul>
      </section>
      <section className="card stack compact panel">
        <h2>Workspace Settings</h2>
        <p>{settings.data?.providerMode ?? "unknown"}</p>
        <p>{settings.data?.language ?? "unknown"}</p>
        <p>{settings.data?.defaultQueryLimit ?? 0}</p>
        <Link to="/settings">Settings</Link>
      </section>
      <section className="card stack compact panel">
        <h2>Entry Points</h2>
        <Link to="/projects">Projects</Link>
      </section>
    </section>
  );
}
