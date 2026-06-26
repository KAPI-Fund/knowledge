import type { ColumnDef } from "@tanstack/react-table";
import { Link, useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { RouteStatePane } from "@/components/layout/route-state-pane";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { normalizeAppError } from "@/lib/app-error";

import { useProjectAuditLogsQuery } from "../audit/queries";
import { useProjectReviewsQuery } from "../reviews/queries";
import { useProjectSourceWatchQuery } from "../source-watch/queries";
import { useProjectSourcesQuery } from "../sources/queries";
import { useProjectTasksQuery } from "../tasks/queries";

import { useProjectDetailQuery } from "./detail-queries";

type SourceRow = NonNullable<ReturnType<typeof useProjectSourcesQuery>["data"]>[number];
type TaskRow = NonNullable<ReturnType<typeof useProjectTasksQuery>["data"]>[number];

const sourceColumns: ColumnDef<SourceRow>[] = [
  {
    accessorKey: "relativePath",
    header: "Path",
    cell: ({ row }) => (
      <span className="font-medium text-foreground">{row.original.relativePath}</span>
    ),
  },
  {
    accessorKey: "size",
    header: "Size",
    cell: ({ row }) => <span className="text-sm text-muted-foreground">{row.original.size}</span>,
  },
];

const taskColumns: ColumnDef<TaskRow>[] = [
  {
    accessorKey: "title",
    header: "Task",
    cell: ({ row }) => <span className="font-medium text-foreground">{row.original.title}</span>,
  },
  {
    id: "status",
    header: "Status",
    cell: ({ row }) => <StatusPill value={row.original.status} />,
  },
];

export function ProjectDetailPage() {
  const { projectId = "" } = useParams();
  const project = useProjectDetailQuery(projectId);
  const sources = useProjectSourcesQuery(projectId);
  const tasks = useProjectTasksQuery(projectId);
  const reviews = useProjectReviewsQuery(projectId, {
    status: "unresolved",
    itemType: "",
    limit: 5,
  });
  const auditLogs = useProjectAuditLogsQuery(projectId);
  const sourceWatch = useProjectSourceWatchQuery(projectId);
  const detail = project.data?.project;

  if (project.isLoading) {
    return <RouteStatePane description="Loading project overview." state="loading" title="Overview" />;
  }

  if (project.error) {
    const normalized = normalizeAppError(project.error);
    return (
      <RouteStatePane
        description={normalized.message}
        state={normalized.kind === "forbidden" || normalized.kind === "not_found" ? normalized.kind : "failed"}
        title="Overview unavailable"
      />
    );
  }

  if (!detail) {
    return (
      <RouteStatePane
        description="Project details are unavailable."
        state="not_found"
        title="Project not found"
      />
    );
  }

  const recentSources = (sources.data ?? []).slice(0, 5);
  const recentTasks = (tasks.data ?? []).slice(0, 5);
  const recentReviews = (reviews.data ?? []).slice(0, 5);
  const recentAudit = (auditLogs.data ?? []).slice(0, 5);

  return (
    <div className="grid gap-6">
      <PageHeader
        description="Operational overview for the current project workspace."
        title="Overview"
      />

      <div className="grid gap-4 md:grid-cols-3">
        <MetricCard label="Sources" value={detail.sourceCount} />
        <MetricCard label="Tasks" value={detail.taskCount} />
        <MetricCard label="Reviews" value={detail.reviewCount} />
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Project Health</CardTitle>
          <CardDescription>Current ingestion watch state and filesystem root.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3 text-sm md:grid-cols-2">
          <div className="space-y-1">
            <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
              Source Watch
            </p>
            <p>{sourceWatch.data?.enabled ? "Source watch enabled" : "Source watch disabled"}</p>
          </div>
          <div className="space-y-1">
            <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
              Auto Ingest
            </p>
            <p>{sourceWatch.data?.autoIngest ? "Ingest tasks are queued automatically." : "Manual ingest only."}</p>
          </div>
          <div className="space-y-1 md:col-span-2">
            <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
              Watch Path
            </p>
            <p>{sourceWatch.data?.path || detail.rootPath}</p>
          </div>
        </CardContent>
      </Card>

      <div className="grid gap-6 xl:grid-cols-2">
        <div className="grid gap-3">
          <h2 className="text-sm font-medium text-muted-foreground">Recent Sources</h2>
          {recentSources.length ? (
            <DataTable columns={sourceColumns} data={recentSources} isLoading={sources.isLoading} />
          ) : (
            <EmptyState
              description="Import sources before running project workflows."
              title="No sources yet"
            />
          )}
        </div>

        <div className="grid gap-3">
          <h2 className="text-sm font-medium text-muted-foreground">Recent Tasks</h2>
          {recentTasks.length ? (
            <DataTable columns={taskColumns} data={recentTasks} isLoading={tasks.isLoading} />
          ) : (
            <EmptyState
              description="No project tasks have been queued yet."
              title="No tasks yet"
            />
          )}
        </div>
      </div>

      <div className="grid gap-4 xl:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Recent Reviews</CardTitle>
            <CardDescription>Outstanding review items from recent processing.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-3">
            {recentReviews.length ? (
              recentReviews.map((review) => (
                <div
                  key={review.id}
                  className="rounded-xl border border-border/70 bg-muted/30 p-4"
                >
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="space-y-1">
                      <p className="font-medium">{review.title}</p>
                      {review.description ? (
                        <p className="text-sm text-muted-foreground">{review.description}</p>
                      ) : null}
                    </div>
                    <StatusPill value={review.status} />
                  </div>
                </div>
              ))
            ) : (
              <EmptyState
                description="Review items will appear here after ingest and lint workflows run."
                title="No reviews yet"
              />
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Recent Audit</CardTitle>
            <CardDescription>Latest workspace mutations recorded by the backend.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-3">
            {recentAudit.length ? (
              recentAudit.map((item) => (
                <div
                  key={item.id}
                  className="rounded-xl border border-border/70 bg-muted/30 p-4"
                >
                  <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                    {item.action}
                  </p>
                  <p className="mt-1 font-medium">{item.summary}</p>
                </div>
              ))
            ) : (
              <EmptyState
                description="Audit entries will appear once project actions have been recorded."
                title="No audit activity"
              />
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Workspace Shortcuts</CardTitle>
          <CardDescription>Jump directly into project operations.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap gap-4 text-sm">
          <Link className="inline-link" to={`/projects/${detail.id}/files`}>
            Open Files
          </Link>
          <Link className="inline-link" to={`/projects/${detail.id}/sources`}>
            Open Sources
          </Link>
          <Link className="inline-link" to={`/projects/${detail.id}/tasks`}>
            Open Tasks
          </Link>
        </CardContent>
      </Card>
    </div>
  );
}

function MetricCard({ label, value }: { label: string; value: number }) {
  return (
    <Card>
      <CardHeader>
        <CardDescription>{label}</CardDescription>
        <CardTitle className="text-2xl font-semibold">{value}</CardTitle>
      </CardHeader>
    </Card>
  );
}
