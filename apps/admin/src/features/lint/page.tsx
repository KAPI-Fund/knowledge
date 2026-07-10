import { useQueryClient } from "@tanstack/react-query";
import type { ColumnDef } from "@tanstack/react-table";
import { Link2, Loader2, SearchCheck, Send, Sparkles, Trash2, Wrench, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import { toast } from "sonner";

import { EmptyState } from "@/components/layout/empty-state";
import { ConfirmDialog } from "@/components/shared/confirm-dialog";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { normalizeAppError } from "@/lib/app-error";

import type { LintItem } from "../shared/api";
import { ProjectFileLink } from "../shared/file-links";

import {
  useCreateLintTaskMutation,
  useDeleteLintOrphanMutation,
  useDismissLintItemsMutation,
  useFixLintItemMutation,
  useLintItemsQuery,
  useLintTaskDetailQuery,
  useSendLintItemsToReviewMutation,
} from "./queries";

// upstream_llm_wiki/src/components/lint/lint-view.tsx typeLabels (i18n en.json lint.typeLabels)
const TYPE_LABELS: Record<string, string> = {
  orphan: "Orphan Page",
  "broken-link": "Broken Link",
  "no-outlinks": "No Outbound Links",
  semantic: "Semantic Issue",
};

const TERMINAL_TASK_STATUSES = ["succeeded", "failed", "cancelled", "completed"];

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

function SuggestionBadge({ item }: { item: LintItem }) {
  const suggestion = item.suggestedSource ?? item.suggestedTarget;
  if (!suggestion) {
    return <Badge variant="outline">→ Review</Badge>;
  }
  return (
    <Badge className="max-w-[260px] border-emerald-500/20 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300">
      <Link2 className="size-3" />
      <span className="truncate font-mono text-[11px]">{suggestion}</span>
    </Badge>
  );
}

export function LintPage() {
  const { projectId = "" } = useParams();
  const queryClient = useQueryClient();
  const createLintTask = useCreateLintTaskMutation();
  const [activeTaskId, setActiveTaskId] = useState("");
  const task = useLintTaskDetailQuery(projectId, activeTaskId);
  const taskStatus = task.data?.status ?? "";
  const taskMode = (task.data?.result as { mode?: string } | undefined | null)?.mode;

  const lintItems = useLintItemsQuery(projectId);
  const items = useMemo(() => lintItems.data ?? [], [lintItems.data]);

  const fixLintItem = useFixLintItemMutation();
  const deleteLintOrphan = useDeleteLintOrphanMutation();
  const dismissLintItems = useDismissLintItemsMutation();
  const sendLintItemsToReview = useSendLintItemsToReviewMutation();

  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [detailId, setDetailId] = useState<string | null>(null);
  const [fixingId, setFixingId] = useState<string | null>(null);
  const [batchFixing, setBatchFixing] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<LintItem | null>(null);

  // react-query v5 has no onSuccess on useQuery: refresh sidecar items when
  // the lint task reaches a terminal state.
  useEffect(() => {
    if (TERMINAL_TASK_STATUSES.includes(taskStatus)) {
      void queryClient.invalidateQueries({ queryKey: ["lint-items", projectId] });
    }
  }, [taskStatus, projectId, queryClient]);

  useEffect(() => {
    const known = new Set(items.map((item) => item.id));
    setSelectedIds((prev) => {
      const next = new Set([...prev].filter((id) => known.has(id)));
      return next.size === prev.size ? prev : next;
    });
    setDetailId((prev) => (prev && known.has(prev) ? prev : null));
  }, [items]);

  const selected = items.find((item) => item.id === detailId) ?? null;
  const selectedItems = useMemo(
    () => items.filter((item) => selectedIds.has(item.id)),
    [items, selectedIds],
  );
  const allSelected = items.length > 0 && selectedItems.length === items.length;
  const isFixing = fixingId !== null || batchFixing;
  const isMutating =
    isFixing ||
    deleteLintOrphan.isPending ||
    dismissLintItems.isPending ||
    sendLintItemsToReview.isPending;

  function setItemSelected(id: string, checked: boolean) {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (checked) {
        next.add(id);
      } else {
        next.delete(id);
      }
      return next;
    });
  }

  function toggleAll() {
    setSelectedIds(allSelected ? new Set() : new Set(items.map((item) => item.id)));
  }

  const columns = useMemo<ColumnDef<LintItem>[]>(
    () => [
      {
        id: "select",
        enableSorting: false,
        header: () => (
          <Checkbox
            aria-label="Select all"
            checked={allSelected ? true : selectedIds.size > 0 ? "indeterminate" : false}
            onCheckedChange={() => toggleAll()}
          />
        ),
        cell: ({ row }) => (
          <Checkbox
            aria-label={`Select ${row.original.page}`}
            checked={selectedIds.has(row.original.id)}
            onCheckedChange={(checked) => setItemSelected(row.original.id, checked === true)}
          />
        ),
      },
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
            onClick={() => setDetailId(row.original.id)}
            type="button"
          >
            {TYPE_LABELS[row.original.issueType] ?? row.original.issueType}
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
        id: "suggestion",
        header: "Suggestion",
        cell: ({ row }) => <SuggestionBadge item={row.original} />,
      },
      {
        accessorKey: "detail",
        header: "Detail",
        cell: ({ row }) => (
          <span className="line-clamp-1 text-sm text-muted-foreground">{row.original.detail}</span>
        ),
      },
    ],
    [allSelected, selectedIds, items],
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

  // upstream_llm_wiki/src/components/lint/lint-view.tsx handleFix wrapper (L170-237)
  async function handleFix(item: LintItem) {
    setFixingId(item.id);
    try {
      const result = await fixLintItem.mutateAsync({ projectId, itemId: item.id });
      if (result.action === "fixed") {
        toast.success(
          result.changedPaths?.length
            ? `Fixed. Updated: ${result.changedPaths.join(", ")}`
            : "Fixed.",
        );
      } else {
        toast.success(`Sent to Review: ${item.page}`);
      }
    } catch (error) {
      toast.error(`Fix failed: ${normalizeAppError(error).message}`);
    } finally {
      setFixingId(null);
    }
  }

  // upstream handleBatchFix: sequential loop (L314-325)
  async function handleBatchFix() {
    if (batchFixing || selectedItems.length === 0) {
      return;
    }
    setBatchFixing(true);
    let fixed = 0;
    let sentToReview = 0;
    try {
      for (const item of selectedItems) {
        try {
          const result = await fixLintItem.mutateAsync({ projectId, itemId: item.id });
          if (result.action === "fixed") {
            fixed += 1;
          } else {
            sentToReview += 1;
          }
        } catch (error) {
          toast.error(`Fix failed for ${item.page}: ${normalizeAppError(error).message}`);
        }
      }
      setSelectedIds(new Set());
      if (fixed || sentToReview) {
        toast.success(
          [fixed ? `Fixed ${fixed} item(s).` : null, sentToReview ? `Sent ${sentToReview} to Review.` : null]
            .filter(Boolean)
            .join(" "),
        );
      }
    } finally {
      setBatchFixing(false);
    }
  }

  async function handleDismiss(ids: string[]) {
    try {
      const result = await dismissLintItems.mutateAsync({ projectId, ids });
      setSelectedIds(new Set());
      toast.success(`Ignored ${result.dismissedIds.length} item(s).`);
    } catch (error) {
      toast.error(normalizeAppError(error).message);
    }
  }

  async function handleSendToReview(ids: string[]) {
    try {
      const result = await sendLintItemsToReview.mutateAsync({ projectId, ids });
      setSelectedIds(new Set());
      toast.success(`Sent ${result.reviewIds.length} item(s) to Review.`);
    } catch (error) {
      toast.error(normalizeAppError(error).message);
    }
  }

  async function handleDeleteOrphan(item: LintItem) {
    try {
      const result = await deleteLintOrphan.mutateAsync({ projectId, itemId: item.id });
      toast.success(`Deleted ${result.deletedPaths.join(", ")}`);
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
        description="Run llm_wiki-style structural and semantic validation, then fix issues in place."
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
            {taskMode ? <Badge variant="outline">{taskMode}</Badge> : null}
          </CardContent>
        </Card>
      ) : null}

      {selectedItems.length > 0 ? (
        <div className="flex flex-wrap items-center gap-2 rounded-md border border-border bg-muted/40 px-3 py-2">
          <span className="text-sm text-muted-foreground">{selectedItems.length} selected</span>
          <Button disabled={isMutating} onClick={() => handleBatchFix()} size="sm" variant="outline">
            {batchFixing ? <Loader2 className="animate-spin" /> : <Wrench />}
            {batchFixing ? "Fixing..." : "Fix selected"}
          </Button>
          <Button
            disabled={isMutating}
            onClick={() => handleSendToReview([...selectedIds])}
            size="sm"
            variant="outline"
          >
            <Send />
            Send selected to Review
          </Button>
          <Button
            className="text-destructive hover:text-destructive"
            disabled={isMutating}
            onClick={() => handleDismiss([...selectedIds])}
            size="sm"
            variant="outline"
          >
            <X />
            Ignore selected
          </Button>
        </div>
      ) : null}

      {items.length ? (
        <div className="grid min-h-0 flex-1 gap-6 xl:grid-cols-[minmax(0,1.4fr)_380px]">
          <DataTable
            columns={columns}
            data={items}
            emptyMessage="No issues found."
            fillHeight
            isLoading={lintItems.isLoading}
          />

          <Card className="flex min-h-0 flex-col">
            <CardHeader>
              <CardTitle>Issue Detail</CardTitle>
              <CardDescription>Selected lint issue, suggestion, and fix actions.</CardDescription>
            </CardHeader>
            <CardContent className="grid min-h-0 flex-1 content-start gap-3 overflow-auto">
              {selected ? (
                <>
                  <div className="flex flex-wrap items-center gap-3">
                    <SeverityBadge severity={selected.severity} />
                    <span className="text-sm font-medium">
                      {TYPE_LABELS[selected.issueType] ?? selected.issueType}
                    </span>
                    <Badge variant="outline">{selected.mode}</Badge>
                  </div>
                  {selected.issueType === "semantic" ? (
                    <p className="text-sm">{selected.page}</p>
                  ) : (
                    <ProjectFileLink projectId={projectId} path={`wiki/${selected.page}`} />
                  )}
                  <p className="text-sm text-muted-foreground">{selected.detail}</p>

                  {selected.suggestedSource || selected.suggestedTarget ? (
                    <div className="rounded-md border border-emerald-500/20 bg-emerald-500/5 px-3 py-2 text-sm text-emerald-700 dark:text-emerald-300">
                      <div className="flex items-start gap-2">
                        <Link2 className="mt-0.5 size-3.5 shrink-0" />
                        <span className="min-w-0 break-all font-medium">
                          {selected.suggestedSource
                            ? `Suggested source: ${selected.suggestedSource}`
                            : `Suggested target: ${selected.suggestedTarget}`}
                        </span>
                      </div>
                    </div>
                  ) : (
                    <p className="text-xs text-muted-foreground">
                      No automatic suggestion — Fix will send this item to Review.
                    </p>
                  )}

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

                  <div className="flex flex-wrap items-center gap-2 pt-1">
                    <Button
                      disabled={isMutating}
                      onClick={() => handleFix(selected)}
                      size="sm"
                      variant="outline"
                    >
                      {fixingId === selected.id ? <Loader2 className="animate-spin" /> : <Wrench />}
                      {fixingId === selected.id ? "Fixing..." : "Fix"}
                    </Button>
                    {selected.issueType === "orphan" ? (
                      <Button
                        className="text-destructive hover:text-destructive"
                        disabled={isMutating}
                        onClick={() => setDeleteTarget(selected)}
                        size="sm"
                        variant="outline"
                      >
                        <Trash2 />
                        Delete
                      </Button>
                    ) : null}
                    <Button
                      disabled={isMutating}
                      onClick={() => handleDismiss([selected.id])}
                      size="sm"
                      variant="ghost"
                    >
                      <X />
                      Dismiss
                    </Button>
                  </div>
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
      ) : lintItems.isLoading ? null : (
        <EmptyState
          description="Run structural or semantic lint to populate this list."
          title="No lint issues"
        />
      )}

      <ConfirmDialog
        confirmLabel="Delete"
        description="The page file, its embeddings, and every reference to it across the wiki will be removed."
        destructive
        isPending={deleteLintOrphan.isPending}
        onConfirm={async () => {
          if (deleteTarget) {
            await handleDeleteOrphan(deleteTarget);
          }
        }}
        onOpenChange={(open) => {
          if (!open) {
            setDeleteTarget(null);
          }
        }}
        open={deleteTarget !== null}
        title={`Delete orphan page "${deleteTarget?.page ?? ""}"?`}
      />
    </div>
  );
}
