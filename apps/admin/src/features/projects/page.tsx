import { ProjectlessState } from "../../components/layout/projectless-state";

import { useProjectsQuery } from "./queries";

export function ProjectsPage() {
  const projects = useProjectsQuery();

  return (
    <section>
      <h1>Projects</h1>
      {projects.data?.length ? (
        <ul>
          {projects.data.map((project) => (
            <li key={project.id}>{project.name}</li>
          ))}
        </ul>
      ) : (
        <ProjectlessState />
      )}
    </section>
  );
}
