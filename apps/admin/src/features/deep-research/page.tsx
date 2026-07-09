import type { ColumnDef } from "@tanstack/react-table";
import { useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";

import { ProjectFileLink } from "../shared/file-links";

import { EmptyState } from "@/components/layout/empty-state";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";

import { useProjectTasksQuery } from "../tasks/queries";

import { useCreateDeepResearchTaskMutation } from "./queries";

type ResearchTask = NonNullable<ReturnType<typeof useProjectTasksQuery>["data"]>[number];

function renderTaskResult(
  task: { result?: unknown; error?: unknown },
  projectId: string,
) {
  if (task.error) {
    const message =
      typeof task.error === "object" && task.error && "message" in task.error
        ? String((task.error as { message?: unknown }).message ?? task.error)
        : String(task.error);
    return <p className="text-destructive">Error: {message}</p>;
  }
  if (
    typeof task.result === "object" &&
    task.result &&
    "savedPath" in task.result
  ) {
    const result = task.result as {
      savedPath?: string;
      sourceCount?: number;
      errors?: string[];
    };
    return (
      <div className="grid gap-1">
        {result.savedPath ? (
          <p>
            Saved: <ProjectFileLink projectId={projectId} path={result.savedPath} />
          </p>
        ) : null}
        {typeof result.sourceCount === "number" ? (
          <p>Sources used: {result.sourceCount}</p>
        ) : null}
        {result.errors && result.errors.length > 0 ? (
          <p className="text-muted-foreground">
            Source errors: {result.errors.join("; ")}
          </p>
        ) : null}
      </div>
    );
  }
  return null;
}

export function DeepResearchPage() {
  const { projectId = "" } = useParams();
  const [topic, setTopic] = useState("");
  const [queries, setQueries] = useState("");
  const createTask = useCreateDeepResearchTaskMutation();
  const tasksQuery = useProjectTasksQuery(projectId);
  const tasks = (tasksQuery.data ?? []).filter((task) => task.taskType === "project.deep_research");
  const [selectedId, setSelectedId] = useState("");

  useEffect(() => {
    if (!selectedId && tasks[0]?.id) {
      setSelectedId(tasks[0].id);
    }
  }, [selectedId, tasks]);

  const selected = tasks.find((task) => task.id === selectedId) ?? null;

  const columns = useMemo<ColumnDef<ResearchTask>[]>(
    () => [
      {
        accessorKey: "status",
        header: "Status",
        cell: ({ row }) => <StatusPill value={row.original.status} />,
      },
      {
        accessorKey: "title",
        header: "Title",
        cell: ({ row }) => (
          <button
            className="text-left font-medium text-foreground underline-offset-4 hover:underline"
            onClick={() => setSelectedId(row.original.id)}
            type="button"
          >
            {row.original.title}
          </button>
        ),
      },
      {
        accessorKey: "updatedAt",
        header: "Updated",
        cell: ({ row }) => (
          <span className="text-sm text-muted-foreground">{row.original.updatedAt ?? "—"}</span>
        ),
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) => (
          <div className="flex justify-end">
            <Button onClick={() => setSelectedId(row.original.id)} size="sm" variant="outline">
              Inspect
            </Button>
          </div>
        ),
      },
    ],
    [],
  );

  return (
    <div className="flex h-full min-h-0 flex-col gap-6">
      <PageHeader
        description="Run a deep research task: fan-out search queries against the configured provider, synthesize a wiki page, save it under wiki/queries/."
        title="Deep Research"
      />

      <Card>
        <CardHeader>
          <CardTitle>New Research</CardTitle>
          <CardDescription>
            The synthesizer uses the configured chat provider. Web search uses the configured search provider (see Settings).
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <label className="grid gap-2 text-sm font-medium">
            Topic
            <Input
              aria-label="Topic"
              onChange={(event) => setTopic(event.target.value)}
              placeholder="e.g. Knowledge Graphs"
              value={topic}
            />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Search Queries
            <Input
              aria-label="Search Queries"
              onChange={(event) => setQueries(event.target.value)}
              placeholder="Comma-separated. Leave blank to use the topic as the only query."
              value={queries}
            />
          </label>
          <div className="flex justify-end">
            <Button
              disabled={createTask.isPending || topic.trim().length === 0}
              onClick={async () => {
                const parsed = queries
                  .split(",")
                  .map((value) => value.trim())
                  .filter((value) => value.length > 0);
                await createTask.mutateAsync({
                  projectId,
                  topic: topic.trim(),
                  searchQueries: parsed.length > 0 ? parsed : undefined,
                });
                setTopic("");
                setQueries("");
              }}
            >
              Start Research
            </Button>
          </div>
        </CardContent>
      </Card>

      <div className="grid min-h-0 flex-1 gap-6 xl:grid-cols-[minmax(0,1.4fr)_380px]">
        <DataTable
          columns={columns}
          data={tasks}
          emptyMessage="No research tasks yet for this project."
          isLoading={tasksQuery.isLoading}
          fillHeight
        />

        <Card className="flex min-h-0 flex-col">
          <CardHeader>
            <CardTitle>Research Detail</CardTitle>
            <CardDescription>Selected research task status and result.</CardDescription>
          </CardHeader>
          <CardContent className="grid min-h-0 flex-1 gap-4 overflow-auto">
            {selected ? (
              <div className="grid gap-3">
                <div className="flex flex-wrap items-center gap-3">
                  <StatusPill value={selected.status} />
                  <span className="text-sm font-medium">{selected.title}</span>
                </div>
                <p className="text-xs text-muted-foreground">
                  Updated {selected.updatedAt ?? "—"} · created {selected.createdAt ?? "—"}
                </p>
                <div className="grid gap-2 text-sm">{renderTaskResult(selected, projectId)}</div>
              </div>
            ) : (
              <EmptyState
                description="Select a research task from the table to inspect its result."
                title="No task selected"
              />
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
