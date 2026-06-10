import { useState } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";

import { ProjectFileLink } from "../shared/file-links";
import { useCancelTaskMutation, useRetryTaskMutation } from "../tasks/queries";

import {
  useCreateQueryTaskMutation,
  useQueryTaskDetailQuery,
  useSaveQueryTaskMutation,
} from "./queries";

type QueryCitation = {
  path: string;
  title: string;
  snippet: string;
  score: number;
};

export function QueryPage() {
  const { projectId = "" } = useParams();
  const createQueryTask = useCreateQueryTaskMutation();
  const retryTask = useRetryTaskMutation();
  const cancelTask = useCancelTaskMutation();
  const saveQueryTask = useSaveQueryTaskMutation();
  const [query, setQuery] = useState("");
  const [topK, setTopK] = useState("3");
  const [activeTaskId, setActiveTaskId] = useState("");
  const [saveTaskId, setSaveTaskId] = useState("");
  const task = useQueryTaskDetailQuery(projectId, activeTaskId);
  const saveTask = useQueryTaskDetailQuery(projectId, saveTaskId);

  async function handleRunQuery() {
    const trimmed = query.trim();
    if (!trimmed) {
      return;
    }

    const created = await createQueryTask.mutateAsync({
      projectId,
      query: trimmed,
      topK: Number(topK) || 3,
    });
    setActiveTaskId(created.taskId);
    setSaveTaskId("");
  }

  async function handleSaveToWiki() {
    if (!task.data) {
      return;
    }

    const title = deriveSaveTitle(query, task.data.title);
    const created = await saveQueryTask.mutateAsync({
      projectId,
      taskId: task.data.id,
      title,
    });
    setSaveTaskId(created.taskId);
  }

  const result = task.data?.result as
    | {
        answer?: string;
        citations?: QueryCitation[];
        contextSummary?: string;
      }
    | undefined
    | null;
  const error = task.data?.error as { message?: string } | undefined | null;
  const isTaskActive = Boolean(activeTaskId);
  const queryTask = task.data;

  return (
    <PageSection
      description="Run question-answering tasks and save accepted answers back into the wiki."
      title="Query"
    >
      <Card>
        <CardHeader>
          <CardTitle>Query Input</CardTitle>
          <CardDescription>Launch an async query task against the current project.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <label className="grid gap-2 text-sm font-medium">
            Query
            <Textarea
              aria-label="Query"
              onChange={(event) => setQuery(event.target.value)}
              rows={5}
              value={query}
            />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Top K
            <Input aria-label="Top K" onChange={(event) => setTopK(event.target.value)} value={topK} />
          </label>
          <div className="flex justify-end">
            <Button disabled={createQueryTask.isPending} onClick={handleRunQuery}>
              Run Query
            </Button>
          </div>
        </CardContent>
      </Card>

      {isTaskActive ? (
        <Card>
          <CardHeader>
            <CardTitle>Task</CardTitle>
            <CardDescription>Current query task status and control actions.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4">
            {queryTask ? (
              <div className="flex flex-wrap items-center gap-3">
                <Badge variant="secondary">{queryTask.status}</Badge>
                <span className="text-sm text-muted-foreground">{queryTask.title}</span>
              </div>
            ) : (
              <EmptyState
                description="The query task is starting up. Status will appear once the backend responds."
                title="Running"
              />
            )}

            {queryTask && queryTask.status === "failed" ? (
              <div className="flex justify-start">
                <Button
                  onClick={() => retryTask.mutateAsync({ projectId, taskId: queryTask.id })}
                  variant="outline"
                >
                  Retry
                </Button>
              </div>
            ) : null}

            {queryTask && !["succeeded", "failed", "cancelled"].includes(queryTask.status) ? (
              <div className="flex justify-start">
                <Button
                  onClick={() => cancelTask.mutateAsync({ projectId, taskId: queryTask.id })}
                  variant="outline"
                >
                  Cancel
                </Button>
              </div>
            ) : null}
          </CardContent>
        </Card>
      ) : null}

      {result?.answer ? (
        <div className="grid gap-4">
          <Card>
            <CardHeader>
              <CardTitle>Answer</CardTitle>
              <CardDescription>The generated answer and cited source pages.</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-4">
              <p className="text-sm leading-6">{result.answer}</p>
              {result.contextSummary ? (
                <p className="text-sm text-muted-foreground">{result.contextSummary}</p>
              ) : null}
              <div className="flex justify-start">
                <Button disabled={saveQueryTask.isPending} onClick={handleSaveToWiki}>
                  Save To Wiki
                </Button>
              </div>
              {saveTask.data?.status === "succeeded" ? (
                <p className="text-sm text-muted-foreground">
                  Saved to wiki{" "}
                  <ProjectFileLink
                    projectId={projectId}
                    path={String(
                      (saveTask.data.result as { relativePath?: string } | undefined | null)
                        ?.relativePath ?? "",
                    )}
                  />
                </p>
              ) : null}
              {saveTask.data?.status === "failed" ? (
                <p className="text-sm text-destructive">Save to wiki failed</p>
              ) : null}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Citations</CardTitle>
              <CardDescription>Source pages referenced by the query answer.</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-3">
              {(result.citations ?? []).length ? (
                (result.citations ?? []).map((citation) => (
                  <Card key={citation.path}>
                    <CardHeader>
                      <CardTitle>{citation.title}</CardTitle>
                      <CardDescription>
                        <ProjectFileLink projectId={projectId} path={citation.path} />
                      </CardDescription>
                    </CardHeader>
                    <CardContent className="grid gap-2 pt-0">
                      <p className="text-sm text-muted-foreground">{citation.snippet}</p>
                      <Badge variant="outline">{`Score ${citation.score}`}</Badge>
                    </CardContent>
                  </Card>
                ))
              ) : (
                <EmptyState
                  description="This answer did not return any citations."
                  title="No citations"
                />
              )}
            </CardContent>
          </Card>
        </div>
      ) : null}

      {error?.message ? (
        <Card>
          <CardHeader>
            <CardTitle>Error</CardTitle>
            <CardDescription>Task execution failed.</CardDescription>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-destructive">{error.message}</p>
          </CardContent>
        </Card>
      ) : null}
    </PageSection>
  );
}

function deriveSaveTitle(query: string, taskTitle: string) {
  const trimmedQuery = query.trim();
  if (trimmedQuery) {
    return trimmedQuery;
  }

  return taskTitle.replace(/^Query:\s*/, "").trim() || "Saved Query";
}
