import type { ColumnDef } from "@tanstack/react-table";
import { useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { RouteStatePane } from "@/components/layout/route-state-pane";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { normalizeAppError } from "@/lib/app-error";

import { ProjectFileLink } from "../shared/file-links";
import {
  useCancelTaskMutation,
  useProjectTasksQuery,
  useRetryTaskMutation,
  useTaskDetailQuery,
} from "./queries";

type ProjectTask = NonNullable<ReturnType<typeof useProjectTasksQuery>["data"]>[number];

export function TasksPage() {
  const { projectId = "" } = useParams();
  const tasks = useProjectTasksQuery(projectId);
  const retryTask = useRetryTaskMutation();
  const cancelTask = useCancelTaskMutation();
  const [selectedTaskId, setSelectedTaskId] = useState("");

  const taskList = tasks.data ?? [];

  useEffect(() => {
    if (!selectedTaskId && tasks.data?.[0]?.id) {
      setSelectedTaskId(tasks.data[0].id);
    }
  }, [selectedTaskId, tasks.data]);

  const detail = useTaskDetailQuery(projectId, selectedTaskId);
  const detailError = detail.error ? normalizeAppError(detail.error) : null;

  const columns = useMemo<ColumnDef<ProjectTask>[]>(
    () => [
      {
        accessorKey: "status",
        header: "Status",
        cell: ({ row }) => <StatusPill value={row.original.status} />,
      },
      {
        accessorKey: "title",
        header: "Task",
        cell: ({ row }) => (
          <div className="grid gap-1">
            <button
              className="text-left font-medium text-foreground underline-offset-4 hover:underline"
              onClick={() => setSelectedTaskId(row.original.id)}
              type="button"
            >
              {row.original.title}
            </button>
            <span className="font-mono text-[10.5px] text-muted-foreground">{row.original.id}</span>
          </div>
        ),
      },
      {
        accessorKey: "taskType",
        header: "Type",
        cell: ({ row }) => (
          <span className="font-mono text-[11.5px] text-muted-foreground">
            {row.original.taskType}
          </span>
        ),
      },
      {
        id: "path",
        header: "Path",
        cell: ({ row }) =>
          row.original.relativePath ? (
            <ProjectFileLink path={row.original.relativePath} projectId={projectId} />
          ) : (
            <span className="text-sm text-muted-foreground">-</span>
          ),
      },
      {
        id: "attempts",
        header: "Attempts",
        cell: ({ row }) => (
          <span className="text-sm text-muted-foreground">
            {(row.original.attemptCount ?? 0).toString()}/
            {(row.original.maxAttempts ?? 0).toString()}
          </span>
        ),
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) => (
          <div className="flex flex-wrap justify-end gap-2">
            <Button onClick={() => setSelectedTaskId(row.original.id)} size="sm" variant="outline">
              Inspect
            </Button>
            <Button
              onClick={() => retryTask.mutateAsync({ projectId, taskId: row.original.id })}
              size="sm"
              variant="secondary"
            >
              Retry
            </Button>
            {!["completed", "succeeded", "failed", "cancelled"].includes(row.original.status) ? (
              <Button
                onClick={() => cancelTask.mutateAsync({ projectId, taskId: row.original.id })}
                size="sm"
                variant="outline"
              >
                Cancel
              </Button>
            ) : null}
          </div>
        ),
      },
    ],
    [cancelTask, projectId, retryTask],
  );

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <PageHeader
        description="Inspect queued work, retry failed jobs, and examine task payloads."
        title="Tasks"
      />
      <div className="grid min-h-0 flex-1 gap-6 xl:grid-cols-[minmax(0,1.4fr)_380px]">
        <div className="flex min-h-0 flex-col gap-3">
          {tasks.error ? (
            <RouteStatePane
              description={normalizeAppError(tasks.error).message}
              state="failed"
              title="Tasks unavailable"
            />
          ) : (
            <DataTable
              columns={columns}
              data={taskList}
              emptyMessage="No tasks have been queued for this project yet."
              isLoading={tasks.isLoading}
              fillHeight
            />
          )}
        </div>

        <Card className="flex min-h-0 flex-col">
          <CardHeader>
            <CardTitle>Task Detail</CardTitle>
            <CardDescription>
              Selected task payload, status, and execution metadata.
            </CardDescription>
          </CardHeader>
          <CardContent className="grid min-h-0 flex-1 gap-4 overflow-auto">
            {!selectedTaskId ? (
              <EmptyState
                description="Select a task from the table to inspect its payload and result."
                title="No task selected"
              />
            ) : detailError ? (
              <RouteStatePane
                description={
                  detailError.kind === "not_found"
                    ? "The selected task disappeared or no longer exists."
                    : detailError.message
                }
                state={detailError.kind === "not_found" ? "not_found" : "failed"}
                title={detailError.kind === "not_found" ? "Task no longer exists" : "Task unavailable"}
              />
            ) : detail.data ? (
              <div className="grid gap-4">
                <div className="flex flex-wrap items-center gap-3">
                  <StatusPill value={detail.data.status} />
                  <span className="text-sm font-medium">{detail.data.title}</span>
                </div>
                <div className="grid gap-2 text-sm text-muted-foreground">
                  <p className="font-mono text-xs">{detail.data.taskType}</p>
                  {detail.data.relativePath ? (
                    <ProjectFileLink path={detail.data.relativePath} projectId={projectId} />
                  ) : null}
                </div>
                {detail.data.error ? (
                  <Card>
                    <CardHeader>
                      <CardTitle>Error</CardTitle>
                    </CardHeader>
                    <CardContent className="pt-0">
                      <pre className="whitespace-pre-wrap break-words text-xs">
                        {JSON.stringify(detail.data.error, null, 2)}
                      </pre>
                    </CardContent>
                  </Card>
                ) : null}
                {detail.data.result ? (
                  <Card>
                    <CardHeader>
                      <CardTitle>Result</CardTitle>
                    </CardHeader>
                    <CardContent className="pt-0">
                      <pre className="whitespace-pre-wrap break-words text-xs">
                        {JSON.stringify(detail.data.result, null, 2)}
                      </pre>
                    </CardContent>
                  </Card>
                ) : null}
                <Card>
                  <CardHeader>
                    <CardTitle>Detail</CardTitle>
                  </CardHeader>
                  <CardContent className="pt-0">
                    <pre className="whitespace-pre-wrap break-words text-xs">
                      {JSON.stringify(detail.data.detail, null, 2)}
                    </pre>
                  </CardContent>
                </Card>
              </div>
            ) : (
              <EmptyState description="Task data is not available yet." title="Loading task detail" />
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
