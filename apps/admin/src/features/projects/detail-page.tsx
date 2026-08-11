import type { ColumnDef } from "@tanstack/react-table";
import {
  ArrowRight,
  ClipboardCheck,
  Database,
  FileText,
  ListTodo,
  type LucideIcon,
} from "lucide-react";
import { Link, useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { RouteStatePane } from "@/components/layout/route-state-pane";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
    cell: ({ row }) => <span className="text-muted-foreground">{row.original.size}</span>,
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

function MetricCard({
  icon: Icon,
  label,
  to,
  value,
}: {
  icon: LucideIcon;
  label: string;
  to: string;
  value: number;
}) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardDescription className="text-sm font-medium text-foreground">{label}</CardDescription>
        <Icon className="size-4 text-muted-foreground" />
      </CardHeader>
      <CardContent className="flex items-end justify-between">
        <p className="text-2xl font-semibold tracking-tight">{value}</p>
        <Button asChild className="text-muted-foreground" size="sm" variant="ghost">
          <Link to={to}>
            View all
            <ArrowRight />
          </Link>
        </Button>
      </CardContent>
    </Card>
  );
}

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
  const base = `/projects/${detail.id}`;

  return (
    <div className="grid gap-6">
      <PageHeader
        description="Operational overview for the current project workspace."
        title="Overview"
      />

      <div className="grid gap-4 md:grid-cols-3">
        <MetricCard icon={Database} label="Sources" to={`${base}/sources`} value={detail.sourceCount} />
        <MetricCard icon={ListTodo} label="Tasks" to={`${base}/tasks`} value={detail.taskCount} />
        <MetricCard
          icon={ClipboardCheck}
          label="Reviews"
          to={`${base}/reviews`}
          value={detail.reviewCount}
        />
      </div>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <div className="grid gap-1">
            <CardTitle>Project Health</CardTitle>
            <CardDescription>Current ingestion watch state and filesystem root.</CardDescription>
          </div>
          <Button asChild size="sm" variant="ghost">
            <Link to={`${base}/source-watch`}>
              Configure
              <ArrowRight />
            </Link>
          </Button>
        </CardHeader>
        <CardContent className="grid gap-4 text-sm md:grid-cols-2">
          <div className="grid gap-1.5">
            <p className="text-xs font-medium text-muted-foreground">Source Watch</p>
            <div>
              <Badge variant={sourceWatch.data?.enabled ? "default" : "secondary"}>
                {sourceWatch.data?.enabled ? "Enabled" : "Disabled"}
              </Badge>
            </div>
          </div>
          <div className="grid gap-1.5">
            <p className="text-xs font-medium text-muted-foreground">Auto Ingest</p>
            <div>
              <Badge variant={sourceWatch.data?.autoIngest ? "default" : "secondary"}>
                {sourceWatch.data?.autoIngest ? "Automatic" : "Manual"}
              </Badge>
            </div>
          </div>
          <div className="grid gap-1.5 md:col-span-2">
            <p className="text-xs font-medium text-muted-foreground">Watch Path</p>
            <p className="font-mono text-xs">{sourceWatch.data?.path || detail.rootPath}</p>
          </div>
        </CardContent>
      </Card>

      <div className="grid gap-4 xl:grid-cols-2">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0">
            <div className="grid gap-1">
              <CardTitle>Recent Sources</CardTitle>
              <CardDescription>Latest imported documents.</CardDescription>
            </div>
            <Button asChild className="text-muted-foreground" size="icon-sm" variant="ghost">
              <Link aria-label="Open Sources" to={`${base}/sources`}>
                <ArrowRight />
              </Link>
            </Button>
          </CardHeader>
          <CardContent>
            {recentSources.length ? (
              <DataTable columns={sourceColumns} data={recentSources} isLoading={sources.isLoading} />
            ) : (
              <EmptyState
                description="Import sources before running project workflows."
                icon={FileText}
                title="No sources yet"
              />
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0">
            <div className="grid gap-1">
              <CardTitle>Recent Tasks</CardTitle>
              <CardDescription>Latest queued and completed work.</CardDescription>
            </div>
            <Button asChild className="text-muted-foreground" size="icon-sm" variant="ghost">
              <Link aria-label="Open Tasks" to={`${base}/tasks`}>
                <ArrowRight />
              </Link>
            </Button>
          </CardHeader>
          <CardContent>
            {recentTasks.length ? (
              <DataTable columns={taskColumns} data={recentTasks} isLoading={tasks.isLoading} />
            ) : (
              <EmptyState
                description="No project tasks have been queued yet."
                icon={ListTodo}
                title="No tasks yet"
              />
            )}
          </CardContent>
        </Card>
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
                <div className="rounded-lg border border-border p-4" key={review.id}>
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="grid gap-1">
                      <p className="text-sm font-medium">{review.title}</p>
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
                icon={ClipboardCheck}
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
                <div className="rounded-lg border border-border p-4" key={item.id}>
                  <p className="font-mono text-xs text-muted-foreground">{item.action}</p>
                  <p className="mt-1 text-sm font-medium">{item.summary}</p>
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
    </div>
  );
}
