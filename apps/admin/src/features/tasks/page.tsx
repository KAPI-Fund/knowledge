import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { RouteStatePane } from "@/components/layout/route-state-pane";
import { StatusBadge } from "@/components/layout/status-badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { normalizeAppError } from "@/lib/app-error";

import { ProjectFileLink } from "../shared/file-links";
import { useCancelTaskMutation, useProjectTasksQuery, useRetryTaskMutation, useTaskDetailQuery } from "./queries";

export function TasksPage() {
  const { projectId = "" } = useParams();
  const tasks = useProjectTasksQuery(projectId);
  const retryTask = useRetryTaskMutation();
  const cancelTask = useCancelTaskMutation();
  const [selectedTaskId, setSelectedTaskId] = useState("");

  useEffect(() => {
    if (!selectedTaskId && tasks.data?.[0]?.id) {
      setSelectedTaskId(tasks.data[0].id);
    }
  }, [selectedTaskId, tasks.data]);

  const detail = useTaskDetailQuery(projectId, selectedTaskId);
  const detailError = detail.error ? normalizeAppError(detail.error) : null;

  return (
    <PageSection
      description="Inspect queued work, retry failed jobs, and examine task payloads."
      title="Tasks"
    >
      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.4fr)_380px]">
        <Card>
          <CardHeader>
            <CardTitle>Task Queue</CardTitle>
            <CardDescription>Project background jobs and their current statuses.</CardDescription>
          </CardHeader>
          <CardContent className="p-0">
            {tasks.isLoading ? (
              <RouteStatePane description="Loading task queue." state="loading" title="Tasks" />
            ) : tasks.error ? (
              <RouteStatePane
                description={normalizeAppError(tasks.error).message}
                state="failed"
                title="Tasks unavailable"
              />
            ) : tasks.data?.length ? (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Task</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Path</TableHead>
                    <TableHead>Attempts</TableHead>
                    <TableHead>Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {tasks.data.map((task) => (
                    <TableRow
                      key={task.id}
                      className={selectedTaskId === task.id ? "bg-muted/40" : undefined}
                    >
                      <TableCell className="font-medium">
                        <button
                          className="text-left font-medium text-foreground underline-offset-4 hover:underline"
                          type="button"
                          onClick={() => setSelectedTaskId(task.id)}
                        >
                          {task.title}
                        </button>
                      </TableCell>
                      <TableCell>
                        <StatusBadge value={task.status} />
                      </TableCell>
                      <TableCell>
                        {task.relativePath ? (
                          <ProjectFileLink projectId={projectId} path={task.relativePath} />
                        ) : (
                          <span className="text-sm text-muted-foreground">-</span>
                        )}
                      </TableCell>
                      <TableCell className="text-sm text-muted-foreground">
                        {(task.attemptCount ?? 0).toString()}/{(task.maxAttempts ?? 0).toString()}
                      </TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-2">
                          <Button
                            onClick={() => setSelectedTaskId(task.id)}
                            size="sm"
                            variant="outline"
                          >
                            Inspect
                          </Button>
                          <Button
                            onClick={() => retryTask.mutateAsync({ projectId, taskId: task.id })}
                            size="sm"
                            variant="secondary"
                          >
                            Retry
                          </Button>
                          {!["completed", "succeeded", "failed", "cancelled"].includes(task.status) ? (
                            <Button
                              onClick={() => cancelTask.mutateAsync({ projectId, taskId: task.id })}
                              size="sm"
                              variant="outline"
                            >
                              Cancel
                            </Button>
                          ) : null}
                        </div>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            ) : (
              <div className="p-6">
                <EmptyState
                  description="No tasks have been queued for this project yet."
                  title="No tasks"
                />
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Task Detail</CardTitle>
            <CardDescription>Selected task payload, status, and execution metadata.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4">
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
                  <StatusBadge value={detail.data.status} />
                  <span className="text-sm font-medium">{detail.data.title}</span>
                </div>
                <div className="grid gap-2 text-sm text-muted-foreground">
                  <p>{detail.data.taskType}</p>
                  {detail.data.relativePath ? (
                    <ProjectFileLink projectId={projectId} path={detail.data.relativePath} />
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
              <EmptyState
                description="Task data is not available yet."
                title="Loading task detail"
              />
            )}
          </CardContent>
        </Card>
      </div>
    </PageSection>
  );
}
