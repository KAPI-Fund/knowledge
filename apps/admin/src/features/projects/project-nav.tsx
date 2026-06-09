import { NavLink } from "react-router-dom";

export function ProjectNav({ projectId }: { projectId: string }) {
  return (
    <nav className="subnav">
      <NavLink to={`/projects/${projectId}`}>Overview</NavLink>
      <NavLink to={`/projects/${projectId}/files`}>Files</NavLink>
      <NavLink to={`/projects/${projectId}/sources`}>Sources</NavLink>
      <NavLink to={`/projects/${projectId}/source-watch`}>Source Watch</NavLink>
      <NavLink to={`/projects/${projectId}/search`}>Search</NavLink>
      <NavLink to={`/projects/${projectId}/query`}>Query</NavLink>
      <NavLink to={`/projects/${projectId}/lint`}>Lint</NavLink>
      <NavLink to={`/projects/${projectId}/graph`}>Graph</NavLink>
      <NavLink to={`/projects/${projectId}/tasks`}>Tasks</NavLink>
      <NavLink to={`/projects/${projectId}/reviews`}>Reviews</NavLink>
      <NavLink to={`/projects/${projectId}/audit`}>Audit</NavLink>
    </nav>
  );
}
