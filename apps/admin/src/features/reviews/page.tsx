import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";
import { useProjectReviewsQuery, useSweepReviewsMutation, useUpdateReviewMutation } from "./queries";

export function ReviewsPage() {
  const { projectId = "" } = useParams();
  const reviews = useProjectReviewsQuery(projectId);
  const sweepReviews = useSweepReviewsMutation();
  const updateReview = useUpdateReviewMutation();

  return (
    <section className="stack">
      <h1>Reviews</h1>
      <ProjectNav projectId={projectId} />
      <button
        type="button"
        onClick={() => sweepReviews.mutateAsync({ projectId })}
      >
        Sweep Reviews
      </button>
      <ul className="results-list">
        {reviews.data?.map((review) => (
          <li key={review.id} className="card stack compact panel">
            <div>
              <strong>{review.title}</strong> <span>{review.status}</span>
            </div>
            {review.type ? <span>{review.type}</span> : null}
            {review.description ? <p>{review.description}</p> : null}
            {review.sourcePath ? <span>{review.sourcePath}</span> : null}
            {review.affectedPages?.length ? (
              <div className="stack compact">
                <strong>Affected Pages</strong>
                <ul>
                  {review.affectedPages.map((page) => (
                    <li key={page}>{page}</li>
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
