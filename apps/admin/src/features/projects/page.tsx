import { Link } from "react-router-dom";
import { useState } from "react";

import { ProjectlessState } from "../../components/layout/projectless-state";

import { useCreateProjectMutation } from "./mutations";
import { useProjectsQuery } from "./queries";

export function ProjectsPage() {
  const projects = useProjectsQuery();
  const createProject = useCreateProjectMutation();
  const [created, setCreated] = useState(false);

  async function handleCreateDemoProject() {
    if (created) {
      return;
    }

    const rootPath = `E:/Projects/Js/knowledge/.worktrees/platform-foundation/.e2e/seed-project-${Date.now()}`;
    await createProject.mutateAsync({
      name: "seed-project",
      rootPath,
      csrfToken: window.sessionStorage.getItem("knowledge.csrfToken") ?? "",
    });
    setCreated(true);
  }

  return (
    <section>
      <h1>Projects</h1>
      <button type="button" onClick={handleCreateDemoProject}>
        Create Demo Project
      </button>
      {projects.data?.length ? (
        <ul>
          {projects.data.map((project) => (
            <li key={project.id}>
              <Link to={`/projects/${project.id}`}>{project.name}</Link>
            </li>
          ))}
        </ul>
      ) : (
        <ProjectlessState />
      )}
    </section>
  );
}
