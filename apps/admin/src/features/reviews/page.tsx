import { useQueryClient } from "@tanstack/react-query";
import type { ColumnDef } from "@tanstack/react-table";
import { Check, ClipboardCheck, RefreshCw, X } from "lucide-react";
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
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { normalizeAppError } from "@/lib/app-error";

import { ProjectFileLink } from "../shared/file-links";
import {
  useProjectReviewsQuery,
  useResolveReviewsMutation,
  useSweepReviewsMutation,
} from "./queries";

type ReviewRow = NonNullable<ReturnType<typeof useProjectReviewsQuery>["data"]>[number];

function canSelectReview(review: ReviewRow) {
  return review.status !== "resolved" && review.status !== "dismissed";
}

export function ReviewsPage() {
  const { projectId = "" } = useParams();
  const queryClient = useQueryClient();
  const [status, setStatus] = useState("unresolved");
  const [itemType, setItemType] = useState("");
  const [limit, setLimit] = useState("200");
  const [selectedReviewId, setSelectedReviewId] = useState("");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const reviews = useProjectReviewsQuery(projectId, {
    status,
    itemType,
    limit: Number(limit) || 200,
  });
  const sweepReviews = useSweepReviewsMutation();
  const resolveReviews = useResolveReviewsMutation();

  const reviewList = useMemo(() => reviews.data ?? [], [reviews.data]);
  const selectableReviews = useMemo(() => reviewList.filter(canSelectReview), [reviewList]);
  const selectedReviews = useMemo(
    () => reviewList.filter((review) => selectedIds.has(review.id)),
    [reviewList, selectedIds],
  );
  const allPendingSelected =
    selectableReviews.length > 0 && selectableReviews.every((review) => selectedIds.has(review.id));

  useEffect(() => {
    if (!selectedReviewId && reviewList[0]?.id) {
      setSelectedReviewId(reviewList[0].id);
    }
  }, [selectedReviewId, reviewList]);

  useEffect(() => {
    const known = new Set(reviewList.map((review) => review.id));
    setSelectedIds((prev) => {
      const next = new Set([...prev].filter((id) => known.has(id)));
      return next.size === prev.size ? prev : next;
    });
    setSelectedReviewId((prev) => (prev && known.has(prev) ? prev : reviewList[0]?.id ?? ""));
  }, [reviewList]);

  const selectedReview = reviewList.find((review) => review.id === selectedReviewId) ?? null;

  function setReviewSelected(id: string, checked: boolean) {
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

  function togglePendingSelection() {
    setSelectedIds(
      allPendingSelected ? new Set() : new Set(selectableReviews.map((review) => review.id)),
    );
  }

  async function handleResolve(ids: string[], action: "resolve" | "dismiss") {
    try {
      const result = await resolveReviews.mutateAsync({ projectId, ids, action });
      setSelectedIds(new Set());
      toast.success(
        action === "dismiss"
          ? `Dismissed ${result.count} review item(s).`
          : `Resolved ${result.count} review item(s).`,
      );
      if (result.notFound.length) {
        toast.warning(`${result.notFound.length} review item(s) were already gone.`);
      }
    } catch (error) {
      toast.error(normalizeAppError(error).message);
    }
  }

  const columns = useMemo<ColumnDef<ReviewRow>[]>(
    () => [
      {
        id: "select",
        enableSorting: false,
        header: () => (
          <Checkbox
            aria-label="Select pending reviews"
            checked={allPendingSelected ? true : selectedIds.size > 0 ? "indeterminate" : false}
            disabled={selectableReviews.length === 0}
            onCheckedChange={() => togglePendingSelection()}
          />
        ),
        cell: ({ row }) => {
          const selectable = canSelectReview(row.original);
          return (
            <Checkbox
              aria-label={`Select ${row.original.title}`}
              checked={selectedIds.has(row.original.id)}
              disabled={!selectable}
              onCheckedChange={(checked) => setReviewSelected(row.original.id, checked === true)}
            />
          );
        },
      },
      {
        accessorKey: "status",
        header: "Status",
        cell: ({ row }) => <StatusPill value={row.original.status} />,
      },
      {
        accessorKey: "type",
        header: "Type",
        cell: ({ row }) =>
          row.original.type ? (
            <Badge variant="outline">{row.original.type}</Badge>
          ) : (
            <span className="text-sm text-muted-foreground">-</span>
          ),
      },
      {
        accessorKey: "title",
        header: "Title",
        cell: ({ row }) => (
          <button
            className="text-left font-medium text-foreground underline-offset-4 hover:underline"
            onClick={() => setSelectedReviewId(row.original.id)}
            type="button"
          >
            {row.original.title}
          </button>
        ),
      },
      {
        id: "source",
        header: "Source",
        cell: ({ row }) =>
          row.original.sourcePath ? (
            <ProjectFileLink path={row.original.sourcePath} projectId={projectId} />
          ) : (
            <span className="text-sm text-muted-foreground">-</span>
          ),
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) => (
          <div className="flex flex-wrap justify-end gap-2">
            <Button onClick={() => setSelectedReviewId(row.original.id)} size="sm" variant="outline">
              Inspect
            </Button>
            {canSelectReview(row.original) ? (
              <Button
                disabled={resolveReviews.isPending}
                onClick={() => handleResolve([row.original.id], "resolve")}
                size="sm"
                variant="secondary"
              >
                Resolve
              </Button>
            ) : null}
          </div>
        ),
      },
    ],
    [
      allPendingSelected,
      projectId,
      resolveReviews.isPending,
      selectableReviews,
      selectedIds,
    ],
  );

  return (
    <div className="grid gap-4">
      <PageHeader
        actions={
          <div className="flex flex-wrap gap-2">
            <Button
              disabled={reviews.isFetching}
              onClick={async () => {
                await queryClient.invalidateQueries({ queryKey: ["project-reviews", projectId] });
                toast.success("Reviews refreshed.");
              }}
              variant="outline"
            >
              <RefreshCw className={reviews.isFetching ? "animate-spin" : undefined} />
              Refresh
            </Button>
            <Button
              disabled={sweepReviews.isPending}
              onClick={async () => {
                try {
                  await sweepReviews.mutateAsync({ projectId });
                  toast.success("Review sweep queued.");
                } catch (error) {
                  toast.error(normalizeAppError(error).message);
                }
              }}
            >
              <ClipboardCheck />
              Sweep Reviews
            </Button>
          </div>
        }
        description="Filter unresolved review items, inspect affected pages, and resolve outcomes."
        title="Reviews"
      />

      <div className="flex flex-wrap items-center gap-2">
        <Input
          aria-label="Filter by type"
          className="h-8 w-40 lg:w-64"
          onChange={(event) => setItemType(event.target.value)}
          placeholder="Filter by type"
          value={itemType}
        />
        <Select onValueChange={setStatus} value={status}>
          <SelectTrigger aria-label="Status" size="sm" className="w-40">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="unresolved">unresolved</SelectItem>
            <SelectItem value="resolved">resolved</SelectItem>
            <SelectItem value="all">all</SelectItem>
          </SelectContent>
        </Select>
        <Input
          aria-label="Limit"
          className="h-8 w-24"
          onChange={(event) => setLimit(event.target.value)}
          value={limit}
        />
      </div>

      {selectedReviews.length > 0 ? (
        <div className="flex flex-wrap items-center gap-2 rounded-md border border-border bg-muted/40 px-3 py-2">
          <span className="text-sm text-muted-foreground">{selectedReviews.length} selected</span>
          <Button
            disabled={resolveReviews.isPending}
            onClick={() => handleResolve([...selectedIds], "resolve")}
            size="sm"
            variant="outline"
          >
            <Check />
            Mark selected resolved
          </Button>
          <Button
            className="text-destructive hover:text-destructive"
            disabled={resolveReviews.isPending}
            onClick={() => handleResolve([...selectedIds], "dismiss")}
            size="sm"
            variant="outline"
          >
            <X />
            Dismiss selected
          </Button>
          <Button onClick={() => setSelectedIds(new Set())} size="sm" variant="ghost">
            Clear selection
          </Button>
        </div>
      ) : null}

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.4fr)_380px]">
        <div className="grid gap-3">
          {reviewList.length ? (
            <DataTable columns={columns} data={reviewList} isLoading={reviews.isLoading} />
          ) : (
            <EmptyState
              description="Review items will appear after the backend runs review generation."
              title="No reviews"
            />
          )}
        </div>

        <Card>
          <CardHeader>
            <CardTitle>Review Detail</CardTitle>
            <CardDescription>Affected pages, search queries, and resolution options.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4">
            {!selectedReview ? (
              <EmptyState
                description="Select a review from the table to inspect its metadata."
                title="No review selected"
              />
            ) : (
              <div className="grid gap-4">
                <div className="flex flex-wrap items-center gap-2">
                  <StatusPill value={selectedReview.status} />
                  {selectedReview.type ? <Badge variant="outline">{selectedReview.type}</Badge> : null}
                </div>
                <p className="text-sm font-medium">{selectedReview.title}</p>
                <p className="text-sm text-muted-foreground">
                  {selectedReview.description ?? "No description provided."}
                </p>
                {selectedReview.affectedPages?.length ? (
                  <div className="grid gap-2">
                    <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                      Affected Pages
                    </p>
                    <ul className="grid gap-2">
                      {selectedReview.affectedPages.map((page) => (
                        <li key={page} className="text-sm">
                          <ProjectFileLink path={page} projectId={projectId} />
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}
                {selectedReview.searchQueries?.length ? (
                  <div className="grid gap-2">
                    <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                      Search Queries
                    </p>
                    <ul className="flex flex-wrap gap-2">
                      {selectedReview.searchQueries.map((query) => (
                        <li key={query}>
                          <Badge variant="outline">{query}</Badge>
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}
                {selectedReview.options?.length ? (
                  <div className="grid gap-2">
                    <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                      Options
                    </p>
                    <ul className="flex flex-wrap gap-2">
                      {selectedReview.options.map((option) => (
                        <li key={`${selectedReview.id}-${option.action}`}>
                          <Badge variant="secondary">{option.label}</Badge>
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
