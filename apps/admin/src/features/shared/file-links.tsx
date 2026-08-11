import type { ReactNode } from "react";
import { Link } from "react-router-dom";

export function projectFilesHref(projectId: string, path: string) {
  const searchParams = new URLSearchParams({
    root: "all",
    path,
  });
  return `/projects/${projectId}/files?${searchParams.toString()}`;
}

export function ProjectFileLink({
  projectId,
  path,
  children,
}: {
  projectId: string;
  path: string;
  children?: ReactNode;
}) {
  return (
    <Link
      className="break-all text-primary underline-offset-4 hover:underline"
      to={projectFilesHref(projectId, path)}
    >
      {children ?? path}
    </Link>
  );
}
