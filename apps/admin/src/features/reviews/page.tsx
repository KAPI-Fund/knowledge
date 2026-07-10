import type { ColumnDef } from "@tanstack/react-table";
import { ClipboardCheck } from "lucide-react";
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
import { useProjectReviewsQuery, useSweepReviewsMutation, useUpdateReviewMutation } from "./queries";

type ReviewRow = NonNullable<ReturnType<typeof useProjectReviewsQuery>["data"]>[number];

export function ReviewsPage() {
  const { projectId = "" } = useParams();
  const [status, setStatus] = useState("unresolved");
  const [itemType, setItemType] = useState("");
  const [limit, setLimit] = useState("200");
  const [selectedReviewId, setSelectedReviewId] = useState("");
  const reviews = useProjectReviewsQuery(projectId, {
    status,
    itemType,
    limit: Number(limit) || 200,
  });
  const sweepReviews = useSweepReviewsMutation();
  const updateReview = useUpdateReviewMutation();

  const reviewList = reviews.data ?? [];

  useEffect(() => {
    if (!selectedReviewId && reviews.data?.[0]?.id) {
      setSelectedReviewId(reviews.data[0].id);
    }
  }, [selectedReviewId, reviews.data]);

  const selectedReview = reviewList.find((review) => review.id === selectedReviewId) ?? null;

  const columns = useMemo<ColumnDef<ReviewRow>[]>(
    () => [
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
            <Button
              onClick={async () => {
                try {
                  await updateReview.mutateAsync({
                    projectId,
                    reviewId: row.original.id,
                    status: "resolved",
                  });
                  toast.success("Review resolved.");
                } catch (error) {
                  toast.error(normalizeAppError(error).message);
                }
              }}
              size="sm"
              variant="secondary"
            >
              Resolve
            </Button>
          </div>
        ),
      },
    ],
    [projectId, updateReview],
  );

  return (
    <div className="grid gap-4">
      <PageHeader
        actions={
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
