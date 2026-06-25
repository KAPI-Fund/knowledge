import { Outlet, useParams } from "react-router-dom";

import { normalizeAppError } from "@/lib/app-error";
import { useProjectDetailQuery } from "@/features/projects/detail-queries";

import { RouteStatePane } from "./route-state-pane";

export function ProjectWorkspaceLayout() {
  const { projectId = "" } = useParams();
  const project = useProjectDetailQuery(projectId);

  if (project.isLoading) {
    return <RouteStatePane description="Loading project context." state="loading" title="Project" />;
  }

  if (project.error) {
    const normalized = normalizeAppError(project.error);
    if (normalized.kind === "forbidden") {
      return (
        <RouteStatePane
          description="You do not have access to this project."
          state="forbidden"
          title="Access denied"
        />
      );
    }
    if (normalized.kind === "not_found") {
      return (
        <RouteStatePane
          description="The requested project no longer exists."
          state="not_found"
          title="Project not found"
        />
      );
    }
    return (
      <RouteStatePane description={normalized.message} state="failed" title="Project unavailable" />
    );
  }

  const detail = project.data?.project;
  if (!detail) {
    return (
      <RouteStatePane
        description="Project details are unavailable."
        state="not_found"
        title="Project not found"
      />
    );
  }

  return <Outlet />;
}
