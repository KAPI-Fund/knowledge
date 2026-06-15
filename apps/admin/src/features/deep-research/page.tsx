import { useState } from "react";
import { useParams } from "react-router-dom";

import { ProjectFileLink } from "../shared/file-links";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { StatusBadge } from "@/components/layout/status-badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";

import { useProjectTasksQuery } from "../tasks/queries";

import { useCreateDeepResearchTaskMutation } from "./queries";

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

  return (
    <PageSection
      description="Run a deep research task: fan-out search queries against the configured provider, synthesize a wiki page, save it under wiki/queries/."
      title="Deep Research"
    >
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

      {tasks.length ? (
        <div className="grid gap-3">
          {tasks.map((task) => (
            <Card key={task.id}>
              <CardHeader>
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="space-y-1">
                    <CardTitle>{task.title}</CardTitle>
                    <CardDescription>
                      Updated {task.updatedAt ?? "—"} · created {task.createdAt ?? "—"}
                    </CardDescription>
                  </div>
                  <StatusBadge value={task.status} />
                </div>
              </CardHeader>
              <CardContent className="grid gap-2 text-sm">
                {renderTaskResult(task, projectId)}
              </CardContent>
            </Card>
          ))}
        </div>
      ) : (
        <EmptyState description="No research tasks yet for this project." title="No research" />
      )}
    </PageSection>
  );
}
