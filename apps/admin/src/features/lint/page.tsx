import { useState } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

import { ProjectFileLink } from "../shared/file-links";

import { useCreateLintTaskMutation, useLintTaskDetailQuery } from "./queries";

type LintIssue = {
  issueType: string;
  severity: string;
  page: string;
  detail: string;
  affectedPages?: string[];
};

export function LintPage() {
  const { projectId = "" } = useParams();
  const createLintTask = useCreateLintTaskMutation();
  const [activeTaskId, setActiveTaskId] = useState("");
  const task = useLintTaskDetailQuery(projectId, activeTaskId);
  const result = task.data?.result as
    | {
        mode?: string;
        issues?: LintIssue[];
      }
    | undefined
    | null;

  async function handleRunStructuralLint() {
    const created = await createLintTask.mutateAsync({
      projectId,
      mode: "structural",
    });
    setActiveTaskId(created.taskId);
  }

  async function handleRunSemanticLint() {
    const created = await createLintTask.mutateAsync({
      projectId,
      mode: "semantic",
    });
    setActiveTaskId(created.taskId);
  }

  return (
    <div className="grid gap-6">
      <PageHeader
        actions={
          <div className="flex flex-wrap gap-2">
            <Button disabled={createLintTask.isPending} onClick={handleRunStructuralLint}>
              Run Structural Lint
            </Button>
            <Button disabled={createLintTask.isPending} onClick={handleRunSemanticLint} variant="outline">
              Run Semantic Lint
            </Button>
          </div>
        }
        description="Run llm_wiki-style structural and semantic validation against the wiki."
        title="Lint"
      />

      {task.data ? (
        <Card>
          <CardHeader>
            <CardTitle>Task</CardTitle>
            <CardDescription>Current lint task status and mode.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-wrap items-center gap-3">
            <StatusPill value={task.data.status} />
            {result?.mode ? <Badge variant="outline">{result.mode}</Badge> : null}
          </CardContent>
        </Card>
      ) : null}

      {result?.issues?.length ? (
        <div className="grid gap-4">
          {result.issues.map((issue, index) => (
            <Card key={`${issue.page}:${issue.issueType}:${index}`}>
              <CardHeader>
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="space-y-1">
                    <CardTitle>{issue.issueType}</CardTitle>
                    <CardDescription>{issue.page}</CardDescription>
                  </div>
                  <Badge variant="secondary">{issue.severity}</Badge>
                </div>
              </CardHeader>
              <CardContent className="grid gap-3">
                {issue.issueType === "semantic" ? <p className="text-sm">{issue.page}</p> : null}
                {issue.issueType !== "semantic" ? (
                  <ProjectFileLink projectId={projectId} path={`wiki/${issue.page}`} />
                ) : null}
                <p className="text-sm text-muted-foreground">{issue.detail}</p>
                {issue.affectedPages?.length ? (
                  <div className="grid gap-2">
                    <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                      Affected Pages
                    </p>
                    <ul className="grid gap-2">
                      {issue.affectedPages.map((page) => (
                        <li key={`${issue.page}:${page}`} className="text-sm">
                          <ProjectFileLink projectId={projectId} path={`wiki/${page}`} />
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}
              </CardContent>
            </Card>
          ))}
        </div>
      ) : task.data ? (
        <EmptyState
          description="The lint task completed without returning any issues."
          title="No issues found"
        />
      ) : null}
    </div>
  );
}
