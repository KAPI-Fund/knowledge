import { zodResolver } from "@hookform/resolvers/zod";
import type { ColumnDef } from "@tanstack/react-table";
import { Telescope } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { useParams } from "react-router-dom";
import { toast } from "sonner";
import { z } from "zod";

import { ProjectFileLink } from "../shared/file-links";

import { EmptyState } from "@/components/layout/empty-state";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { normalizeAppError } from "@/lib/app-error";
import { formatDateTime } from "@/lib/format";

import { useProjectTasksQuery } from "../tasks/queries";

import { useCreateDeepResearchTaskMutation } from "./queries";

type ResearchTask = NonNullable<ReturnType<typeof useProjectTasksQuery>["data"]>[number];

const researchSchema = z.object({
  topic: z.string().trim().min(1, "Topic is required."),
  queries: z.string(),
});

type ResearchValues = z.infer<typeof researchSchema>;

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
  const createTask = useCreateDeepResearchTaskMutation();
  const tasksQuery = useProjectTasksQuery(projectId);
  const tasks = (tasksQuery.data ?? []).filter((task) => task.taskType === "project.deep_research");
  const [selectedId, setSelectedId] = useState("");

  const form = useForm<ResearchValues>({
    resolver: zodResolver(researchSchema),
    defaultValues: { topic: "", queries: "" },
  });

  useEffect(() => {
    if (!selectedId && tasks[0]?.id) {
      setSelectedId(tasks[0].id);
    }
  }, [selectedId, tasks]);

  const selected = tasks.find((task) => task.id === selectedId) ?? null;

  async function onSubmit(values: ResearchValues) {
    const parsed = values.queries
      .split(",")
      .map((value) => value.trim())
      .filter((value) => value.length > 0);
    try {
      await createTask.mutateAsync({
        projectId,
        topic: values.topic.trim(),
        searchQueries: parsed.length > 0 ? parsed : undefined,
      });
      toast.success("Research task queued.");
      form.reset();
    } catch (error) {
      toast.error(normalizeAppError(error).message);
    }
  }

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
          <span className="text-sm text-muted-foreground">
            {row.original.updatedAt ? formatDateTime(row.original.updatedAt) : "—"}
          </span>
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
        <CardContent>
          <Form {...form}>
            <form className="grid gap-4" onSubmit={form.handleSubmit(onSubmit)}>
              <FormField
                control={form.control}
                name="topic"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Topic</FormLabel>
                    <FormControl>
                      <Input placeholder="e.g. Knowledge Graphs" {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="queries"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Search Queries</FormLabel>
                    <FormControl>
                      <Input
                        placeholder="Comma-separated. Leave blank to use the topic as the only query."
                        {...field}
                      />
                    </FormControl>
                    <FormDescription>
                      Each query fans out to the search provider before synthesis.
                    </FormDescription>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <div className="flex justify-end">
                <Button disabled={createTask.isPending} type="submit">
                  <Telescope />
                  Start Research
                </Button>
              </div>
            </form>
          </Form>
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
                  Updated {selected.updatedAt ? formatDateTime(selected.updatedAt) : "—"} · created{" "}
                  {selected.createdAt ? formatDateTime(selected.createdAt) : "—"}
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
