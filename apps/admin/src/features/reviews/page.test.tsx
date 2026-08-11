import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ReviewsPage } from "./page";

const mockReviews = vi.fn();
const mockResolveReviews = vi.fn();
const mockSweepReviews = vi.fn();

vi.mock("./queries", () => ({
  useProjectReviewsQuery: () => ({
    data: mockReviews(),
    isLoading: false,
    isFetching: false,
  }),
  useResolveReviewsMutation: () => ({
    mutateAsync: mockResolveReviews,
    isPending: false,
  }),
  useSweepReviewsMutation: () => ({
    mutateAsync: mockSweepReviews,
    isPending: false,
  }),
}));

const openReview = {
  id: "review-11111111",
  status: "open",
  type: "missing-page",
  title: "Missing page: Attention",
  description: "Referenced but missing.",
  sourcePath: "wiki/concepts/attention.md",
  affectedPages: ["wiki/concepts/attention.md"],
  searchQueries: ["attention"],
  options: [{ action: "create", label: "Create page" }],
};

const pendingReview = {
  id: "review-22222222",
  status: "pending",
  type: "duplicate",
  title: "Duplicate page: Delta",
  description: null,
  sourcePath: null,
  affectedPages: [],
  searchQueries: [],
  options: [],
};

const resolvedReview = {
  id: "review-33333333",
  status: "resolved",
  type: "missing-page",
  title: "Missing page: Softmax",
  description: null,
  sourcePath: null,
  affectedPages: [],
  searchQueries: [],
  options: [],
};

function renderPage() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/projects/project-1/reviews"]}>
        <Routes>
          <Route path="projects/:projectId/reviews" element={<ReviewsPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("reviews page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockReviews.mockReturnValue([openReview, pendingReview, resolvedReview]);
  });

  it("selects only pending reviews from the header checkbox", async () => {
    const user = userEvent.setup();

    renderPage();

    expect(
      screen.getByRole("checkbox", { name: "Select Missing page: Softmax" }),
    ).toBeDisabled();

    await user.click(screen.getByRole("checkbox", { name: "Select pending reviews" }));

    expect(screen.getByText("2 selected")).toBeInTheDocument();
  });

  it("marks selected reviews resolved", async () => {
    const user = userEvent.setup();
    mockResolveReviews.mockResolvedValue({
      resolved: ["review-11111111", "review-22222222"],
      notFound: [],
      count: 2,
    });

    renderPage();

    await user.click(screen.getByRole("checkbox", { name: "Select pending reviews" }));
    await user.click(screen.getByRole("button", { name: "Mark selected resolved" }));

    expect(mockResolveReviews).toHaveBeenCalledWith({
      projectId: "project-1",
      ids: ["review-11111111", "review-22222222"],
      action: "resolve",
    });
  });

  it("dismisses selected reviews", async () => {
    const user = userEvent.setup();
    mockResolveReviews.mockResolvedValue({
      resolved: ["review-11111111"],
      notFound: [],
      count: 1,
    });

    renderPage();

    await user.click(
      screen.getByRole("checkbox", { name: "Select Missing page: Attention" }),
    );
    await user.click(screen.getByRole("button", { name: "Dismiss selected" }));

    expect(mockResolveReviews).toHaveBeenCalledWith({
      projectId: "project-1",
      ids: ["review-11111111"],
      action: "dismiss",
    });
  });

  it("resolves a single review from the row action", async () => {
    const user = userEvent.setup();
    mockResolveReviews.mockResolvedValue({
      resolved: ["review-11111111"],
      notFound: [],
      count: 1,
    });

    renderPage();

    const resolveButtons = screen.getAllByRole("button", { name: "Resolve" });
    expect(resolveButtons).toHaveLength(2);
    await user.click(resolveButtons[0]);

    expect(mockResolveReviews).toHaveBeenCalledWith({
      projectId: "project-1",
      ids: ["review-11111111"],
      action: "resolve",
    });
  });

  it("queues a review sweep", async () => {
    const user = userEvent.setup();
    mockSweepReviews.mockResolvedValue({ taskId: "task-1", status: "queued" });

    renderPage();

    await user.click(screen.getByRole("button", { name: "Sweep Reviews" }));

    expect(mockSweepReviews).toHaveBeenCalledWith({ projectId: "project-1" });
  });
});
