import type { ColumnDef } from "@tanstack/react-table";
import { SearchCheck, Sparkles } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import { toast } from "sonner";

import { EmptyState } from "@/components/layout/empty-state";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { normalizeAppError } from "@/lib/app-error";

import { ProjectFileLink } from "../shared/file-links";

import { useCreateLintTaskMutation, useLintTaskDetailQuery } from "./queries";

type LintIssue = {
  issueType: string;
  severity: string;
  page: string;
  detail: string;
  affectedPages?: string[];
};

function SeverityBadge({ severity }: { severity: string }) {
  if (severity === "error") {
    return <Badge variant="destructive">{severity}</Badge>;
  }
  if (severity === "warning") {
    return (
      <Badge className="border-transparent bg-amber-100 text-amber-900 dark:bg-amber-950 dark:text-amber-200">
        {severity}
      </Badge>
    );
  }
  return <Badge variant="secondary">{severity}</Badge>;
}

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

  const issues = result?.issues ?? [];
  const [selectedIndex, setSelectedIndex] = useState(0);

  useEffect(() => {
    setSelectedIndex(0);
  }, [activeTaskId]);

  const selected = issues[selectedIndex] ?? null;

  const columns = useMemo<ColumnDef<LintIssue>[]>(
    () => [
      {
        accessorKey: "severity",
        header: "Severity",
        cell: ({ row }) => <SeverityBadge severity={row.original.severity} />,
      },
      {
        accessorKey: "issueType",
        header: "Type",
        cell: ({ row }) => (
          <button
            className="text-left font-medium text-foreground underline-offset-4 hover:underline"
            onClick={() => setSelectedIndex(row.index)}
            type="button"
          >
            {row.original.issueType}
          </button>
        ),
      },
      {
        accessorKey: "page",
        header: "Page",
        cell: ({ row }) => (
          <span className="font-mono text-[11.5px] text-muted-foreground">{row.original.page}</span>
        ),
      },
      {
        accessorKey: "detail",
        header: "Detail",
        cell: ({ row }) => (
          <span className="line-clamp-1 text-sm text-muted-foreground">{row.original.detail}</span>
        ),
      },
    ],
    [],
  );

  async function handleRunLint(mode: "structural" | "semantic") {
    try {
      const created = await createLintTask.mutateAsync({ projectId, mode });
      setActiveTaskId(created.taskId);
      toast.success(mode === "structural" ? "Structural lint queued." : "Semantic lint queued.");
    } catch (error) {
      toast.error(normalizeAppError(error).message);
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-6">
      <PageHeader
        actions={
          <div className="flex flex-wrap gap-2">
            <Button disabled={createLintTask.isPending} onClick={() => handleRunLint("structural")}>
              <SearchCheck />
              Run Structural Lint
            </Button>
            <Button
              disabled={createLintTask.isPending}
              onClick={() => handleRunLint("semantic")}
              variant="outline"
            >
              <Sparkles />
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
        <div className="grid min-h-0 flex-1 gap-6 xl:grid-cols-[minmax(0,1.4fr)_380px]">
          <DataTable columns={columns} data={issues} emptyMessage="No issues found." fillHeight />

          <Card className="flex min-h-0 flex-col">
            <CardHeader>
              <CardTitle>Issue Detail</CardTitle>
              <CardDescription>Selected lint issue and affected pages.</CardDescription>
            </CardHeader>
            <CardContent className="grid min-h-0 flex-1 gap-3 overflow-auto">
              {selected ? (
                <>
                  <div className="flex flex-wrap items-center gap-3">
                    <SeverityBadge severity={selected.severity} />
                    <span className="text-sm font-medium">{selected.issueType}</span>
                  </div>
                  {selected.issueType === "semantic" ? (
                    <p className="text-sm">{selected.page}</p>
                  ) : (
                    <ProjectFileLink projectId={projectId} path={`wiki/${selected.page}`} />
                  )}
                  <p className="text-sm text-muted-foreground">{selected.detail}</p>
                  {selected.affectedPages?.length ? (
                    <div className="grid gap-2">
                      <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                        Affected Pages
                      </p>
                      <ul className="grid gap-2">
                        {selected.affectedPages.map((page) => (
                          <li key={`${selected.page}:${page}`} className="text-sm">
                            <ProjectFileLink projectId={projectId} path={`wiki/${page}`} />
                          </li>
                        ))}
                      </ul>
                    </div>
                  ) : null}
                </>
              ) : (
                <EmptyState
                  description="Select an issue from the table to see its detail."
                  title="No issue selected"
                />
              )}
            </CardContent>
          </Card>
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
