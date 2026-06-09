# Project Operations Workbench Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the project detail page into a usable operations workbench that summarizes project health and exposes recent sources, tasks, reviews, and audit activity from the existing APIs.

**Architecture:** Keep the current route structure and data-fetching hooks. Build the workbench as a single project-detail composition layer that aggregates the existing project detail, source, task, review, and audit queries into one coherent dashboard. Avoid new backend endpoints; the page should be a thin presentation layer over the already-available APIs.

**Tech Stack:** React, TypeScript, TanStack Query, React Router, Vitest, Testing Library

---

Use `@superpowers:test-driven-development` for behavior-bearing tasks and `@superpowers:verification-before-completion` before each commit or chunk handoff.

## Chunk 1: Project Operations Workbench

**Chunk goal:** Make the project detail page a real operations hub with live summaries and quick access to the existing project workflows.

**Planned file structure:**

- Modify: `apps/admin/src/features/projects/detail-page.tsx`
- Modify: `apps/admin/src/features/projects/detail-page.test.tsx`
- Modify: `apps/admin/src/features/projects/detail-queries.ts`
- Modify: `apps/admin/src/features/sources/queries.ts`
- Modify: `apps/admin/src/features/tasks/queries.ts`
- Modify: `apps/admin/src/features/reviews/queries.ts`
- Modify: `apps/admin/src/features/audit/queries.ts`

### Task 1: Add A Failing Project Workbench Test

**Files:**
- Modify: `apps/admin/src/features/projects/detail-page.test.tsx`

- [ ] **Step 1: Write the failing workbench test**

```tsx
it("shows project health and recent operational records on the detail page", async () => {
  render(<ProjectDetailPage />);

  expect(await screen.findByRole("heading", { name: "demo-project" })).toBeInTheDocument();
  expect(screen.getByText("2 sources")).toBeInTheDocument();
  expect(screen.getByText("3 tasks")).toBeInTheDocument();
  expect(screen.getByText("1 reviews")).toBeInTheDocument();
  expect(screen.getByText("Recent Sources")).toBeInTheDocument();
  expect(screen.getByText("raw/sources/demo.md")).toBeInTheDocument();
  expect(screen.getByText("Recent Tasks")).toBeInTheDocument();
  expect(screen.getByText("Imported note.md")).toBeInTheDocument();
  expect(screen.getByText("Recent Reviews")).toBeInTheDocument();
  expect(screen.getByText("Review demo.md")).toBeInTheDocument();
  expect(screen.getByText("Recent Audit")).toBeInTheDocument();
  expect(screen.getByText("Created project demo-project")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the new test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- detail-page.test.tsx`
Expected: FAIL because the project detail page only renders counts and nav.

- [ ] **Step 3: Implement the workbench layout**

Requirements:
- Keep the existing project summary and nav.
- Add recent sources, tasks, reviews, and audit panels.
- Keep each panel on live query hooks, no mocked data in production.

- [ ] **Step 4: Re-run the test and verify it passes**

Run: `npm run test --workspace @knowledge/admin -- detail-page.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit the workbench UI**

```bash
git add apps/admin/src/features/projects
git add docs/superpowers/plans/2026-06-09-project-operations-workbench.md
git commit -m "feat: add project operations workbench"
```
