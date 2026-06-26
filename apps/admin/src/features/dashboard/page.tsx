import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { Link } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useSystemSettingsQuery } from "../settings/queries";
import { useProjectsQuery } from "../projects/queries";

type ProjectRow = NonNullable<ReturnType<typeof useProjectsQuery>["data"]>[number];

export function DashboardPage() {
  const projects = useProjectsQuery();
  const settings = useSystemSettingsQuery();
  const projectList = projects.data ?? [];
  const recentProjects = projectList.slice(0, 5);

  const columns = useMemo<ColumnDef<ProjectRow>[]>(
    () => [
      {
        accessorKey: "name",
        header: "Name",
        cell: ({ row }) => (
          <Link
            className="font-medium text-foreground underline-offset-4 hover:underline"
            to={`/projects/${row.original.id}`}
          >
            {row.original.name}
          </Link>
        ),
      },
      {
        accessorKey: "rootPath",
        header: "Root Path",
        cell: ({ row }) => (
          <span className="font-mono text-[11px] text-muted-foreground">{row.original.rootPath}</span>
        ),
      },
    ],
    [],
  );

  return (
    <div className="grid gap-6">
      <PageHeader description="Workspace overview" title="Dashboard" />

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader>
            <CardDescription>Projects</CardDescription>
            <CardTitle className="text-2xl font-semibold">{projectList.length}</CardTitle>
          </CardHeader>
        </Card>
        <Card>
          <CardHeader>
            <CardDescription>Language</CardDescription>
            <CardTitle className="text-2xl font-semibold">{settings.data?.language ?? "unknown"}</CardTitle>
          </CardHeader>
        </Card>
        <Card>
          <CardHeader>
            <CardDescription>Default Query Limit</CardDescription>
            <CardTitle className="text-2xl font-semibold">{settings.data?.defaultQueryLimit ?? 0}</CardTitle>
          </CardHeader>
        </Card>
      </div>

      {recentProjects.length ? (
        <DataTable
          columns={columns}
          data={recentProjects}
          isLoading={projects.isLoading}
        />
      ) : (
        <EmptyState
          action={
            <Link className="inline-link" to="/projects">
              Open Projects
            </Link>
          }
          description="Create a project before importing sources or running retrieval workflows."
          title="Workspace not configured"
        />
      )}

      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Workspace Settings</CardTitle>
            <CardDescription>Current provider and query defaults.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-2 text-sm text-muted-foreground">
            <p>{settings.data?.providerMode ?? "unknown"}</p>
            <p>{settings.data?.language ?? "unknown"}</p>
            <p>{settings.data?.defaultQueryLimit ?? 0}</p>
            <Link className="inline-link" to="/settings">
              Settings
            </Link>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Entry Points</CardTitle>
            <CardDescription>Start from the workspace list or system controls.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-2 text-sm">
            <Link className="inline-link" to="/projects">
              Projects
            </Link>
            <Link className="inline-link" to="/settings">
              Settings
            </Link>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
