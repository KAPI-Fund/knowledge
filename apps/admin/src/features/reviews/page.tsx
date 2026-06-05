import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";
import { useProjectReviewsQuery, useUpdateReviewMutation } from "./queries";

export function ReviewsPage() {
  const { projectId = "" } = useParams();
  const reviews = useProjectReviewsQuery(projectId);
  const updateReview = useUpdateReviewMutation();

  return (
    <section className="stack">
      <h1>Reviews</h1>
      <ProjectNav projectId={projectId} />
      <ul>
        {reviews.data?.map((review) => (
          <li key={review.id}>
            <strong>{review.title}</strong> <span>{review.status}</span>
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
