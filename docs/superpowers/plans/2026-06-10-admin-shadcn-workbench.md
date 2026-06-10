# Admin Shadcn Workbench Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild `apps/admin` into a shadcn-based, route-centered operations console while keeping every current admin route and API-driven workflow functional.

**Architecture:** The implementation keeps the existing route tree, React Query hooks, and backend API contracts intact while replacing the presentation layer with a Tailwind/shadcn component system, authenticated shell, project-scoped workspace layout, and normalized route/action error handling. Shared primitives, layout units, and error adapters are added first so the route pages can migrate onto one consistent structure instead of each page inventing its own UI and failure behavior.

**Tech Stack:** React 19, React Router 7, TanStack Query 5, Vite 7, TypeScript 5, Tailwind CSS 4, shadcn/ui-style components, `@knowledge/api-client`, Vitest, Testing Library

---

## File Structure Map

### New or Expanded Shared Frontend Files

- `apps/admin/components.json`
  - shadcn generator/config metadata for the admin app
- `apps/admin/src/index.css`
  - Tailwind entry, theme tokens, base component layer, shell/page utility classes
- `apps/admin/src/lib/utils.ts`
  - `cn()` helper for class merging
- `apps/admin/src/lib/app-error.ts`
  - normalized admin-facing error kinds and mappers
- `apps/admin/src/lib/route-meta.ts`
  - breadcrumb metadata and project-route detection helpers
- `apps/admin/src/components/ui/*`
  - shared shadcn-style primitives such as button, input, textarea, select, card, badge, tabs, dialog, table, alert, scroll-area, skeleton, separator, tooltip, sheet
- `apps/admin/src/components/layout/auth-guard.tsx`
  - route gate that redirects unauthenticated users to `/login`
- `apps/admin/src/components/layout/system-sidebar.tsx`
  - system navigation for dashboard, projects, users, settings
- `apps/admin/src/components/layout/top-bar.tsx`
  - breadcrumb, user identity, global action region
- `apps/admin/src/components/layout/project-workspace-layout.tsx`
  - project-scoped route container and project context owner
- `apps/admin/src/components/layout/project-header.tsx`
  - project summary header
- `apps/admin/src/components/layout/page-section.tsx`
  - consistent section framing
- `apps/admin/src/components/layout/empty-state.tsx`
  - intentional empty states
- `apps/admin/src/components/layout/route-state-pane.tsx`
  - route-level loading/empty/forbidden/not-found/failed surface
- `apps/admin/src/components/layout/status-badge.tsx`
  - shared badge rendering for task/review/settings states
- `apps/admin/src/components/layout/data-table.tsx`
  - thin wrapper for repeated list/table framing if needed

### Existing Files to Modify

- `apps/admin/package.json`
  - add styling and shadcn dependencies
- `apps/admin/vite.config.ts`
  - add Tailwind Vite plugin and aliases
- `apps/admin/tsconfig.json`
  - add path aliases used by shared UI/layout code
- `apps/admin/src/main.tsx`
  - switch from `styles.css` to `index.css`
- `apps/admin/src/app/router.tsx`
  - add auth guard and nested project workspace layout
- `apps/admin/src/components/layout/app-shell.tsx`
  - rebuild into persistent authenticated shell
- `apps/admin/src/features/shared/api.ts`
  - omit blank `providerApiKey`, keep existing data contracts
- `apps/admin/src/features/auth/use-session.ts`
  - align with normalized client errors
- `apps/admin/src/features/auth/login-page.tsx`
  - migrate to shadcn form card and inline errors
- `apps/admin/src/features/dashboard/page.tsx`
  - migrate to KPI cards and route-centered overview
- `apps/admin/src/features/projects/page.tsx`
  - replace demo-project shortcut with real create-project dialog flow
- `apps/admin/src/features/projects/project-nav.tsx`
  - rebuild as project route tabs/segmented nav
- `apps/admin/src/features/projects/detail-page.tsx`
  - slim into overview page inside project workspace layout
- `apps/admin/src/features/files/page.tsx`
  - migrate to split-pane tree/preview layout
- `apps/admin/src/features/sources/page.tsx`
  - migrate import/upload/table actions
- `apps/admin/src/features/source-watch/page.tsx`
  - migrate full source-watch form, including `autoIngest`
- `apps/admin/src/features/search/page.tsx`
  - migrate search form, summary chips, result cards
- `apps/admin/src/features/query/page.tsx`
  - migrate async task workbench UI
- `apps/admin/src/features/lint/page.tsx`
  - migrate lint action state + results
- `apps/admin/src/features/graph/page.tsx`
  - migrate graph filter/list/detail layout
- `apps/admin/src/features/tasks/page.tsx`
  - migrate task table + detail drawer/panel
- `apps/admin/src/features/reviews/page.tsx`
  - migrate filterable review workbench
- `apps/admin/src/features/audit/page.tsx`
  - migrate audit table + route mapping
- `apps/admin/src/features/users/page.tsx`
  - migrate read-only user list UI
- `apps/admin/src/features/settings/page.tsx`
  - migrate settings form and key-preservation semantics
- `packages/api-client/src/http.ts`
  - throw normalized HTTP errors instead of blindly Zod-parsing non-2xx bodies

### New or Updated Tests

- `packages/api-client/src/http.test.ts`
  - verifies typed client errors and JSON success parsing
- `apps/admin/src/app/router.test.tsx`
  - verifies auth redirects and project-layout routing
- `apps/admin/src/components/layout/app-shell.test.tsx`
  - verifies shell chrome and project/workspace behavior
- `apps/admin/src/features/settings/page.test.tsx`
  - verifies masked key semantics and settings save flow
- `apps/admin/src/features/tasks/page.test.tsx`
  - verifies task detail and retry/cancel rendering
- `apps/admin/src/features/projects/operations-pages.test.tsx`
  - updated to match new workspace chrome and navigation
- `apps/admin/src/features/projects/operations-actions.test.tsx`
  - updated to match new action surfaces
- existing focused route tests such as:
  - `apps/admin/src/features/auth/login-page.test.tsx`
  - `apps/admin/src/features/dashboard/page.test.tsx`
  - `apps/admin/src/features/projects/page.test.tsx`
  - `apps/admin/src/features/query/page.test.tsx`
  - `apps/admin/src/features/lint/page.test.tsx`

## Chunk 1: Foundation And System Routes

### Task 1: Normalize HTTP Errors For The Admin Client

**Files:**
- Create: `packages/api-client/src/http.test.ts`
- Create: `apps/admin/src/lib/app-error.ts`
- Modify: `packages/api-client/src/http.ts`
- Modify: `apps/admin/src/features/auth/use-session.ts`
- Modify: `apps/admin/src/features/shared/api.ts`
- Test: `packages/api-client/src/http.test.ts`

- [ ] **Step 1: Write the failing client-error tests**

```ts
import { describe, expect, it, vi } from "vitest";
import { z } from "zod";
import { apiFetch, ApiClientError } from "./http";

describe("apiFetch", () => {
  it("throws ApiClientError for non-2xx JSON responses", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ error: "unknown project" }), { status: 400 }),
    ));

    await expect(apiFetch("/api/projects/missing", { method: "GET" }, z.any())).rejects.toMatchObject({
      name: "ApiClientError",
      status: 400,
      message: "unknown project",
    });
  });
});
```

- [ ] **Step 2: Run the package test to verify it fails**

Run: `npm run test --workspace @knowledge/api-client -- src/http.test.ts`

Expected: FAIL because `ApiClientError` does not exist and `apiFetch` still tries to Zod-parse error payloads.

- [ ] **Step 3: Implement typed HTTP errors and admin normalization**

```ts
export class ApiClientError extends Error {
  constructor(
    public readonly status: number,
    public readonly body: unknown,
    message: string,
  ) {
    super(message);
    this.name = "ApiClientError";
  }
}

if (!response.ok) {
  throw new ApiClientError(response.status, json, extractErrorMessage(json, response.statusText));
}
```

```ts
export function normalizeAppError(error: unknown): AppError {
  if (error instanceof ApiClientError) {
    if (error.status === 401) return { kind: "auth", status: 401, message: error.message };
    if (error.status === 403) return { kind: "forbidden", status: 403, message: error.message };
    if (error.status === 404 || isUnknownProject(error) || isUnknownTask(error)) {
      return { kind: "not_found", status: error.status, message: error.message };
    }
    return { kind: "failed", status: error.status, message: error.message };
  }
  return { kind: "failed", status: 500, message: "Unexpected request failure" };
}
```

- [ ] **Step 4: Run the package and focused admin tests**

Run: `npm run test --workspace @knowledge/api-client -- src/http.test.ts`

Expected: PASS

Run: `npm run test --workspace @knowledge/admin -- src/features/auth/login-page.test.tsx`

Expected: PASS, confirming the new error path does not break login/session consumers.

- [ ] **Step 5: Commit**

```bash
git add packages/api-client/src/http.ts packages/api-client/src/http.test.ts apps/admin/src/lib/app-error.ts apps/admin/src/features/auth/use-session.ts apps/admin/src/features/shared/api.ts
git commit -m "feat: normalize admin http errors"
```

### Task 2: Install Tailwind/Shadcn Foundation And Shared UI Primitives

**Files:**
- Create: `apps/admin/components.json`
- Create: `apps/admin/src/index.css`
- Create: `apps/admin/src/lib/utils.ts`
- Create: `apps/admin/src/components/ui/button.tsx`
- Create: `apps/admin/src/components/ui/input.tsx`
- Create: `apps/admin/src/components/ui/textarea.tsx`
- Create: `apps/admin/src/components/ui/select.tsx`
- Create: `apps/admin/src/components/ui/card.tsx`
- Create: `apps/admin/src/components/ui/badge.tsx`
- Create: `apps/admin/src/components/ui/dialog.tsx`
- Create: `apps/admin/src/components/ui/tabs.tsx`
- Create: `apps/admin/src/components/ui/table.tsx`
- Create: `apps/admin/src/components/ui/alert.tsx`
- Create: `apps/admin/src/components/ui/scroll-area.tsx`
- Create: `apps/admin/src/components/ui/sheet.tsx`
- Create: `apps/admin/src/components/ui/skeleton.tsx`
- Create: `apps/admin/src/components/ui/separator.tsx`
- Create: `apps/admin/src/components/ui/tooltip.tsx`
- Modify: `apps/admin/package.json`
- Modify: `apps/admin/vite.config.ts`
- Modify: `apps/admin/tsconfig.json`
- Modify: `apps/admin/src/main.tsx`
- Test: `apps/admin/src/components/layout/app-shell.test.tsx`

- [ ] **Step 1: Write the failing shell rendering test**

```tsx
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { AppShell } from "./app-shell";

it("renders system navigation and top-level shell regions", () => {
  render(
    <MemoryRouter>
      <AppShell />
    </MemoryRouter>,
  );

  expect(screen.getByRole("navigation", { name: /system/i })).toBeInTheDocument();
  expect(screen.getByRole("banner")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the admin test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- src/components/layout/app-shell.test.tsx`

Expected: FAIL because the new shell regions and imported UI primitives do not exist yet.

- [ ] **Step 3: Add styling/tooling dependencies and implement primitives**

```ts
// apps/admin/vite.config.ts
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": resolve(__dirname, "./src"),
    },
  },
});
```

```ts
// apps/admin/src/lib/utils.ts
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

- [ ] **Step 4: Run lint and the focused shell test**

Run: `npm run lint --workspace @knowledge/admin`

Expected: PASS

Run: `npm run test --workspace @knowledge/admin -- src/components/layout/app-shell.test.tsx`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/admin/package.json apps/admin/vite.config.ts apps/admin/tsconfig.json apps/admin/components.json apps/admin/src/main.tsx apps/admin/src/index.css apps/admin/src/lib/utils.ts apps/admin/src/components/ui apps/admin/src/components/layout/app-shell.test.tsx
git commit -m "feat: add admin shadcn foundation"
```

### Task 3: Build Authenticated Shell, Route Metadata, And Project Workspace Layout

**Files:**
- Create: `apps/admin/src/lib/route-meta.ts`
- Create: `apps/admin/src/components/layout/auth-guard.tsx`
- Create: `apps/admin/src/components/layout/system-sidebar.tsx`
- Create: `apps/admin/src/components/layout/top-bar.tsx`
- Create: `apps/admin/src/components/layout/project-workspace-layout.tsx`
- Create: `apps/admin/src/components/layout/project-header.tsx`
- Create: `apps/admin/src/components/layout/page-section.tsx`
- Create: `apps/admin/src/components/layout/empty-state.tsx`
- Create: `apps/admin/src/components/layout/route-state-pane.tsx`
- Create: `apps/admin/src/components/layout/status-badge.tsx`
- Modify: `apps/admin/src/components/layout/app-shell.tsx`
- Modify: `apps/admin/src/app/router.tsx`
- Modify: `apps/admin/src/features/projects/project-nav.tsx`
- Modify: `apps/admin/src/features/projects/detail-queries.ts`
- Create: `apps/admin/src/app/router.test.tsx`
- Test: `apps/admin/src/app/router.test.tsx`
- Test: `apps/admin/src/components/layout/app-shell.test.tsx`

- [ ] **Step 1: Write the failing routing/auth tests**

```tsx
it("redirects unauthenticated users to /login", async () => {
  vi.mocked(useSession).mockReturnValue({ user: null });
  render(<AppRouter />);
  expect(await screen.findByRole("heading", { name: /sign in/i })).toBeInTheDocument();
});

it("shows project workspace chrome on /projects/:projectId routes", async () => {
  renderWithRoute("/projects/project-1/files");
  expect(await screen.findByRole("tab", { name: /files/i })).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the focused router and shell tests**

Run: `npm run test --workspace @knowledge/admin -- src/app/router.test.tsx src/components/layout/app-shell.test.tsx`

Expected: FAIL because there is no auth guard, route metadata, or project workspace layout.

- [ ] **Step 3: Implement the authenticated shell and project route container**

```tsx
// apps/admin/src/app/router.tsx
<Route element={<AuthGuard />}>
  <Route path="/" element={<AppShell />}>
    <Route index element={<DashboardPage />} />
    <Route path="projects" element={<ProjectsPage />} />
    <Route path="projects/:projectId" element={<ProjectWorkspaceLayout />}>
      <Route index element={<ProjectDetailPage />} />
      <Route path="files" element={<FilesPage />} />
      // ...
    </Route>
  </Route>
</Route>
```

```tsx
// apps/admin/src/components/layout/project-workspace-layout.tsx
const project = useProjectDetailQuery(projectId);
const routeState = deriveProjectRouteState(project);

return (
  <div className="flex flex-1 flex-col">
    <ProjectHeader />
    <ProjectNav projectId={projectId} />
    <Outlet context={projectContextValue} />
  </div>
);
```

- [ ] **Step 4: Run the router/shell tests**

Run: `npm run test --workspace @knowledge/admin -- src/app/router.test.tsx src/components/layout/app-shell.test.tsx`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/lib/route-meta.ts apps/admin/src/components/layout apps/admin/src/app/router.tsx apps/admin/src/features/projects/project-nav.tsx apps/admin/src/features/projects/detail-queries.ts apps/admin/src/app/router.test.tsx apps/admin/src/components/layout/app-shell.tsx
git commit -m "feat: add admin auth shell and project workspace layout"
```

### Task 4: Migrate Login, Dashboard, Projects, Users, And Settings

**Files:**
- Modify: `apps/admin/src/features/auth/login-page.tsx`
- Modify: `apps/admin/src/features/dashboard/page.tsx`
- Modify: `apps/admin/src/features/projects/page.tsx`
- Modify: `apps/admin/src/features/users/page.tsx`
- Modify: `apps/admin/src/features/settings/page.tsx`
- Modify: `apps/admin/src/features/shared/api.ts`
- Create: `apps/admin/src/features/settings/page.test.tsx`
- Modify: `apps/admin/src/features/auth/login-page.test.tsx`
- Modify: `apps/admin/src/features/dashboard/page.test.tsx`
- Modify: `apps/admin/src/features/projects/page.test.tsx`
- Test: `apps/admin/src/features/auth/login-page.test.tsx`
- Test: `apps/admin/src/features/dashboard/page.test.tsx`
- Test: `apps/admin/src/features/projects/page.test.tsx`
- Test: `apps/admin/src/features/settings/page.test.tsx`

- [ ] **Step 1: Write the failing system-route tests**

```tsx
it("omits providerApiKey when the settings field is blank", async () => {
  render(<SettingsPage />);
  await user.click(screen.getByRole("button", { name: /save settings/i }));
  expect(updateSettings).toHaveBeenCalledWith(expect.not.objectContaining({ providerApiKey: "" }));
});

it("opens a real create-project form instead of the demo shortcut", async () => {
  render(<ProjectsPage />);
  expect(screen.getByRole("button", { name: /create project/i })).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the focused system-route tests**

Run: `npm run test --workspace @knowledge/admin -- src/features/auth/login-page.test.tsx src/features/dashboard/page.test.tsx src/features/projects/page.test.tsx src/features/settings/page.test.tsx`

Expected: FAIL because the pages still use the plain scaffold UI and the settings request still sends the key field directly.

- [ ] **Step 3: Implement the migrated system pages**

```tsx
// apps/admin/src/features/projects/page.tsx
<PageSection
  title="Projects"
  actions={<Button onClick={() => setCreateOpen(true)}>Create Project</Button>}
>
  {projects.length ? <ProjectsTable projects={projects} /> : <EmptyState ... />}
  <CreateProjectDialog />
</PageSection>
```

```ts
// apps/admin/src/features/shared/api.ts
const payload = {
  providerMode: input.providerMode,
  language: input.language,
  defaultQueryLimit: input.defaultQueryLimit,
  providerBaseUrl: input.providerBaseUrl,
  ...(input.providerApiKey?.trim() ? { providerApiKey: input.providerApiKey.trim() } : {}),
};
```

- [ ] **Step 4: Run the focused tests**

Run: `npm run test --workspace @knowledge/admin -- src/features/auth/login-page.test.tsx src/features/dashboard/page.test.tsx src/features/projects/page.test.tsx src/features/settings/page.test.tsx`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/auth/login-page.tsx apps/admin/src/features/dashboard/page.tsx apps/admin/src/features/projects/page.tsx apps/admin/src/features/users/page.tsx apps/admin/src/features/settings/page.tsx apps/admin/src/features/shared/api.ts apps/admin/src/features/auth/login-page.test.tsx apps/admin/src/features/dashboard/page.test.tsx apps/admin/src/features/projects/page.test.tsx apps/admin/src/features/settings/page.test.tsx
git commit -m "feat: migrate admin system routes to shadcn ui"
```

## Chunk 2: Project Workbench And Operations Routes

### Task 5: Migrate Project Overview, Files, Sources, And Source Watch

**Files:**
- Modify: `apps/admin/src/features/projects/detail-page.tsx`
- Modify: `apps/admin/src/features/files/page.tsx`
- Modify: `apps/admin/src/features/sources/page.tsx`
- Modify: `apps/admin/src/features/source-watch/page.tsx`
- Modify: `apps/admin/src/features/projects/operations-pages.test.tsx`
- Modify: `apps/admin/src/features/projects/operations-actions.test.tsx`
- Test: `apps/admin/src/features/projects/operations-pages.test.tsx`
- Test: `apps/admin/src/features/projects/operations-actions.test.tsx`

- [ ] **Step 1: Extend the project operations tests to the new layout**

```tsx
it("renders project overview cards and route tabs inside the workspace layout", async () => {
  renderProjectRoute("/projects/project-1");
  expect(await screen.findByRole("heading", { name: "demo-project" })).toBeInTheDocument();
  expect(screen.getByRole("tab", { name: /sources/i })).toBeInTheDocument();
});

it("keeps source-watch auto-ingest and scan controls wired", async () => {
  renderProjectRoute("/projects/project-1/source-watch");
  expect(screen.getByLabelText(/automatically enqueue ingest tasks/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the project workspace page tests**

Run: `npm run test --workspace @knowledge/admin -- src/features/projects/operations-pages.test.tsx src/features/projects/operations-actions.test.tsx`

Expected: FAIL because the pages still render the old scaffold and do not match the new project workspace framing.

- [ ] **Step 3: Implement the migrated overview/files/sources/source-watch pages**

```tsx
// apps/admin/src/features/files/page.tsx
<div className="grid gap-6 xl:grid-cols-[320px_minmax(0,1fr)]">
  <Card>
    <ScrollArea><FileTreePanel ... /></ScrollArea>
  </Card>
  <Card><FilePreviewPanel ... /></Card>
</div>
```

```tsx
// apps/admin/src/features/sources/page.tsx
<Tabs defaultValue="text-import">
  <TabsList>...</TabsList>
  <TabsContent value="text-import"><TextImportForm /></TabsContent>
  <TabsContent value="upload"><UploadSourcesForm /></TabsContent>
  <TabsContent value="folder"><FolderImportForm /></TabsContent>
</Tabs>
```

- [ ] **Step 4: Run the workspace tests again**

Run: `npm run test --workspace @knowledge/admin -- src/features/projects/operations-pages.test.tsx src/features/projects/operations-actions.test.tsx`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/projects/detail-page.tsx apps/admin/src/features/files/page.tsx apps/admin/src/features/sources/page.tsx apps/admin/src/features/source-watch/page.tsx apps/admin/src/features/projects/operations-pages.test.tsx apps/admin/src/features/projects/operations-actions.test.tsx
git commit -m "feat: migrate admin project workspace pages"
```

### Task 6: Migrate Search, Query, Lint, And Graph With Route-State Vs Action-State Rules

**Files:**
- Modify: `apps/admin/src/features/search/page.tsx`
- Modify: `apps/admin/src/features/query/page.tsx`
- Modify: `apps/admin/src/features/lint/page.tsx`
- Modify: `apps/admin/src/features/graph/page.tsx`
- Modify: `apps/admin/src/features/query/page.test.tsx`
- Modify: `apps/admin/src/features/lint/page.test.tsx`
- Create: `apps/admin/src/features/tasks/page.test.tsx`
- Test: `apps/admin/src/features/query/page.test.tsx`
- Test: `apps/admin/src/features/lint/page.test.tsx`
- Test: `apps/admin/src/features/tasks/page.test.tsx`

- [ ] **Step 1: Write/extend the failing operation-route tests**

```tsx
it("keeps query page chrome visible while a task is pending", async () => {
  renderProjectRoute("/projects/project-1/query");
  await user.click(screen.getByRole("button", { name: /run query/i }));
  expect(screen.getByRole("heading", { name: /query/i })).toBeInTheDocument();
  expect(screen.getByText(/running/i)).toBeInTheDocument();
});

it("shows a local not-found message when a selected task disappears", async () => {
  useTaskDetailQuery.mockReturnValue({ error: makeNotFoundError("unknown task") });
  renderProjectRoute("/projects/project-1/tasks");
  expect(screen.getByText(/task no longer exists/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the focused tests**

Run: `npm run test --workspace @knowledge/admin -- src/features/query/page.test.tsx src/features/lint/page.test.tsx src/features/tasks/page.test.tsx`

Expected: FAIL because the pages do not yet implement the split route/action state behavior or the new task detail UI.

- [ ] **Step 3: Implement the migrated operation pages**

```tsx
// apps/admin/src/features/query/page.tsx
<PageSection title="Query">
  <Card><QueryForm ... /></Card>
  <TaskStatusCard task={task.data} />
  <QueryResultCard result={result} saveTask={saveTask.data} />
</PageSection>
```

```tsx
// apps/admin/src/features/tasks/page.tsx
<div className="grid gap-6 xl:grid-cols-[minmax(0,1.4fr)_380px]">
  <TasksTable ... />
  <TaskDetailCard state={detailState} ... />
</div>
```

- [ ] **Step 4: Run the focused operation tests**

Run: `npm run test --workspace @knowledge/admin -- src/features/query/page.test.tsx src/features/lint/page.test.tsx src/features/tasks/page.test.tsx`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/search/page.tsx apps/admin/src/features/query/page.tsx apps/admin/src/features/lint/page.tsx apps/admin/src/features/graph/page.tsx apps/admin/src/features/tasks/page.tsx apps/admin/src/features/query/page.test.tsx apps/admin/src/features/lint/page.test.tsx apps/admin/src/features/tasks/page.test.tsx
git commit -m "feat: migrate admin operations routes"
```

### Task 7: Migrate Reviews And Audit, Then Verify End-To-End Admin Coverage

**Files:**
- Modify: `apps/admin/src/features/reviews/page.tsx`
- Modify: `apps/admin/src/features/audit/page.tsx`
- Modify: `apps/admin/src/features/projects/operations-pages.test.tsx`
- Modify: `apps/admin/src/features/projects/operations-actions.test.tsx`
- Modify: `apps/admin/src/app/router.test.tsx`
- Test: `apps/admin/src/features/projects/operations-pages.test.tsx`
- Test: `apps/admin/src/features/projects/operations-actions.test.tsx`
- Test: `apps/admin/src/app/router.test.tsx`

- [ ] **Step 1: Extend the failing tests for reviews/audit route behavior**

```tsx
it("renders review filters for status, type, and limit", async () => {
  renderProjectRoute("/projects/project-1/reviews");
  expect(await screen.findByLabelText(/status/i)).toBeInTheDocument();
  expect(screen.getByLabelText(/type/i)).toBeInTheDocument();
  expect(screen.getByLabelText(/limit/i)).toBeInTheDocument();
});

it("renders audit rows without dead links when target mapping is unavailable", async () => {
  renderProjectRoute("/projects/project-1/audit");
  expect(screen.queryByRole("link", { name: /batch/i })).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run the focused route tests**

Run: `npm run test --workspace @knowledge/admin -- src/features/projects/operations-pages.test.tsx src/features/projects/operations-actions.test.tsx src/app/router.test.tsx`

Expected: FAIL because reviews/audit are still using the bare list UI and audit route mapping is not implemented.

- [ ] **Step 3: Implement reviews/audit migration and final route-state handling**

```tsx
// apps/admin/src/features/reviews/page.tsx
<PageSection title="Reviews" actions={<Button ...>Sweep Reviews</Button>}>
  <Card><ReviewFilters ... /></Card>
  <ReviewsTable ... />
</PageSection>
```

```tsx
// apps/admin/src/features/audit/page.tsx
const href = mapAuditItemToRoute(item, projectId);
return href ? <Link to={href}>{item.summary}</Link> : <span>{item.summary}</span>;
```

- [ ] **Step 4: Run the final focused admin verification**

Run: `npm run test --workspace @knowledge/admin`

Expected: PASS

Run: `npm run lint --workspace @knowledge/admin`

Expected: PASS

Run: `npm run build --workspace @knowledge/admin`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/reviews/page.tsx apps/admin/src/features/audit/page.tsx apps/admin/src/features/projects/operations-pages.test.tsx apps/admin/src/features/projects/operations-actions.test.tsx apps/admin/src/app/router.test.tsx
git commit -m "feat: finish admin shadcn workbench migration"
```

