import { useState } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { StatusBadge } from "@/components/layout/status-badge";

import { ProjectFileLink } from "../shared/file-links";
import { useProjectReviewsQuery, useSweepReviewsMutation, useUpdateReviewMutation } from "./queries";

export function ReviewsPage() {
  const { projectId = "" } = useParams();
  const [status, setStatus] = useState("unresolved");
  const [itemType, setItemType] = useState("");
  const [limit, setLimit] = useState("200");
  const reviews = useProjectReviewsQuery(projectId, {
    status,
    itemType,
    limit: Number(limit) || 200,
  });
  const sweepReviews = useSweepReviewsMutation();
  const updateReview = useUpdateReviewMutation();

  return (
    <PageSection
      actions={
        <Button disabled={sweepReviews.isPending} onClick={() => sweepReviews.mutateAsync({ projectId })}>
          Sweep Reviews
        </Button>
      }
      description="Filter unresolved review items, inspect affected pages, and resolve outcomes."
      title="Reviews"
    >
      <Card>
        <CardHeader>
          <CardTitle>Review Filters</CardTitle>
          <CardDescription>Scope review items by status, type, and page limit.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-[180px_minmax(0,1fr)_160px] md:items-end">
          <label className="grid gap-2 text-sm font-medium">
            Status
            <Select aria-label="Status" onChange={(event) => setStatus(event.target.value)} value={status}>
              <option value="unresolved">unresolved</option>
              <option value="resolved">resolved</option>
              <option value="all">all</option>
            </Select>
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Type
            <Input aria-label="Type" onChange={(event) => setItemType(event.target.value)} value={itemType} />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Limit
            <Input aria-label="Limit" onChange={(event) => setLimit(event.target.value)} value={limit} />
          </label>
        </CardContent>
      </Card>

      {reviews.data?.length ? (
        <div className="grid gap-4">
          {reviews.data.map((review) => (
            <Card key={review.id}>
              <CardHeader>
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="space-y-1">
                    <CardTitle>{review.title}</CardTitle>
                    <CardDescription>{review.description ?? "No description provided."}</CardDescription>
                  </div>
                  <StatusBadge value={review.status} />
                </div>
              </CardHeader>
              <CardContent className="grid gap-4">
                <div className="flex flex-wrap gap-2">
                  {review.type ? <Badge variant="outline">{review.type}</Badge> : null}
                </div>
                {review.sourcePath ? (
                  <ProjectFileLink projectId={projectId} path={review.sourcePath} />
                ) : null}
                {review.affectedPages?.length ? (
                  <div className="grid gap-2">
                    <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                      Affected Pages
                    </p>
                    <ul className="grid gap-2">
                      {review.affectedPages.map((page) => (
                        <li key={page} className="text-sm">
                          <ProjectFileLink projectId={projectId} path={page} />
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}
                {review.searchQueries?.length ? (
                  <div className="grid gap-2">
                    <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                      Search Queries
                    </p>
                    <ul className="flex flex-wrap gap-2">
                      {review.searchQueries.map((query) => (
                        <li key={query}>
                          <Badge variant="outline">{query}</Badge>
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}
                {review.options?.length ? (
                  <div className="grid gap-2">
                    <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                      Options
                    </p>
                    <ul className="flex flex-wrap gap-2">
                      {review.options.map((option) => (
                        <li key={`${review.id}-${option.action}`}>
                          <Badge variant="secondary">{option.label}</Badge>
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}
                <div className="flex justify-start">
                  <Button
                    onClick={() =>
                      updateReview.mutateAsync({
                        projectId,
                        reviewId: review.id,
                        status: "resolved",
                      })
                    }
                    variant="outline"
                  >
                    Resolve
                  </Button>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      ) : (
        <EmptyState
          description="Review items will appear after the backend runs review generation."
          title="No reviews"
        />
      )}
    </PageSection>
  );
}
