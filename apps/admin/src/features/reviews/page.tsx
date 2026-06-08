import { useState } from "react";
import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";
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
    <section className="stack">
      <h1>Reviews</h1>
      <ProjectNav projectId={projectId} />
      <div className="card stack compact panel">
        <label>
          Status
          <select value={status} onChange={(event) => setStatus(event.target.value)}>
            <option value="unresolved">unresolved</option>
            <option value="resolved">resolved</option>
            <option value="all">all</option>
          </select>
        </label>
        <label>
          Type
          <input value={itemType} onChange={(event) => setItemType(event.target.value)} />
        </label>
        <label>
          Limit
          <input value={limit} onChange={(event) => setLimit(event.target.value)} />
        </label>
        <button
          type="button"
          onClick={() => sweepReviews.mutateAsync({ projectId })}
        >
          Sweep Reviews
        </button>
      </div>
      <ul className="results-list">
        {reviews.data?.map((review) => (
          <li key={review.id} className="card stack compact panel">
            <div>
              <strong>{review.title}</strong> <span>{review.status}</span>
            </div>
            {review.type ? <span>{review.type}</span> : null}
            {review.description ? <p>{review.description}</p> : null}
            {review.sourcePath ? (
              <ProjectFileLink projectId={projectId} path={review.sourcePath} />
            ) : null}
            {review.affectedPages?.length ? (
              <div className="stack compact">
                <strong>Affected Pages</strong>
                <ul>
                  {review.affectedPages.map((page) => (
                    <li key={page}>
                      <ProjectFileLink projectId={projectId} path={page} />
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
            {review.searchQueries?.length ? (
              <div className="stack compact">
                <strong>Search Queries</strong>
                <ul>
                  {review.searchQueries.map((query) => (
                    <li key={query}>{query}</li>
                  ))}
                </ul>
              </div>
            ) : null}
            {review.options?.length ? (
              <div className="stack compact">
                <strong>Options</strong>
                <ul>
                  {review.options.map((option) => (
                    <li key={`${review.id}-${option.action}`}>{option.label}</li>
                  ))}
                </ul>
              </div>
            ) : null}
            <button
              type="button"
              onClick={() =>
                updateReview.mutateAsync({
                  projectId,
                  reviewId: review.id,
                  status: "resolved",
                })
              }
            >
              Resolve
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
