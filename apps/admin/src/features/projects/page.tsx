import { Link, useNavigate } from "react-router-dom";
import { useState } from "react";

import { ProjectlessState } from "../../components/layout/projectless-state";

import { useCreateProjectMutation } from "./mutations";
import { useProjectsQuery } from "./queries";

export function ProjectsPage() {
  const navigate = useNavigate();
  const projects = useProjectsQuery();
  const createProject = useCreateProjectMutation();
  const [created, setCreated] = useState(false);
  const [demoProjectName] = useState(() => `seed-project-${Date.now()}`);

  async function handleCreateDemoProject() {
    if (created) {
      return;
    }

    const rootPath = buildDemoProjectRootPath(demoProjectName);
    const project = await createProject.mutateAsync({
      name: demoProjectName,
      rootPath,
      csrfToken: window.sessionStorage.getItem("knowledge.csrfToken") ?? "",
    });
    setCreated(true);
    navigate(`/projects/${project.id}`);
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

function buildDemoProjectRootPath(projectName: string) {
  const seed = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const root = import.meta.env.VITE_KNOWLEDGE_PROJECT_ROOT
    ?? (import.meta.env.PROD ? "/data/projects" : "E:/Projects/Js/knowledge/.e2e");
  return `${root}/${projectName}-${seed}`;
}
