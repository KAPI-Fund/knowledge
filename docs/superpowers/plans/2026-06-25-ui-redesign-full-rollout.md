# UI Redesign Full Rollout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface the org/team/project creation flows in the new shell and migrate every remaining admin screen onto the shared "Technical Console" design system so the whole app looks finished and consistent.

**Architecture:** The Foundation work already built a shared design layer (`PageHeader`, `DataTable`, `StatusPill`, `FilterToolbar`, `states.tsx`) and a context-switching sidebar shell, but migrated only the Tasks screen. The redesign also orphaned the org-creation entry point (`SpaceSwitcher` + `CreateOrgDialog` are mounted nowhere). This plan (1) re-homes creation flows into the new sidebar/pages and (2) rolls the established pattern across all remaining screens, screen by screen, preserving existing test assertions.

**Tech Stack:** React 19, React Router 7, TanStack Query 5, @tanstack/react-table 8, Tailwind v4, official `radix-ui` shadcn primitives, lucide-react, vitest 4 + @testing-library.

---

## Conventions (apply to every task)

- **CWD is `apps/admin`.** All `git add` / test commands are relative to `apps/admin`. Use forward slashes.
- **Scripts:** `npm run lint` = `tsc --noEmit` (type check, NOT eslint). `npm test` = `vitest run`. Run BOTH after each task.
- **Compose shadcn primitives** from `@/components/ui/*`; never hand-roll equivalents (project memory rule).
- **Imports:** prefer the `@/` path alias over relative `../../` for `components/*`, `lib/*`, `features/*`. When you touch a file that still uses relative imports, convert the lines you edit to `@/`.
- **ASCII only** in source files (no smart quotes, em-dashes, ellipsis glyphs — use `-`, `...`).
- **Commit per task** with the exact message given. Stage only the files the task touched (`git add src/...`).
- **Light theme only**, blue `#1F6FEB` accent, cool slate neutrals (already encoded in `index.css` tokens; do not invent new colors — use existing tokens / the exact hexes already used by `StatusPill` and `DataTable`).

---

## Migration Recipe (the pattern every screen migration follows)

The reference implementation is `src/features/tasks/page.tsx`. A "migration" of a screen means applying these transforms while keeping behavior and test assertions intact:

1. **Header:** Replace the old `PageSection` / raw `<header><h1>` with `PageHeader` from `@/components/shared/page-header`.
   - `title` = the screen name (keep the SAME visible string the tests assert, e.g. `"Users"`, `"Overview"`, `"Tasks"`).
   - `description` = the existing description string (keep wording where a test asserts it).
   - `actions` = the primary buttons that were in the old header (e.g. "Rescan Sources", "Sweep Reviews", "New team").
   - Wrap the page in `<div className="grid gap-6">` (or `gap-4`) so the header and body stack consistently.
2. **Tables:** Replace raw `@/components/ui/table` usage with `DataTable` from `@/components/shared/data-table`, defining `columns: ColumnDef<Row>[]` with `useMemo`. Map each existing `<TableHead>`/`<TableCell>` to a column. Use:
   - `StatusPill` (`@/components/shared/status-pill`) for any status/role/state cell that was a `StatusBadge`/`Badge`.
   - `font-mono text-[11px] text-muted-foreground` for ids/paths/prefixes.
   - `ProjectFileLink` (`@/features/shared/file-links`) for project-relative file paths.
   - Pass `isLoading`, `emptyMessage`.
3. **Search/filter:** If the screen filters a list, use `FilterToolbar` (`@/components/shared/filter-toolbar`) above the table instead of a bare `<Input>`.
4. **Loading / error / empty:**
   - Query loading for a whole page region -> `DataTable isLoading` (tables) or `LoadingState` (`@/components/shared/states`).
   - Query error -> `RouteStatePane` (`@/components/layout/route-state-pane`) with `state="failed"` (and `not_found` / `forbidden` where the old code distinguished them).
   - Empty list -> `DataTable emptyMessage` or `EmptyState` (`@/components/layout/empty-state`).
   - Replace ad-hoc `<p>Loading...</p>` / `<p>Access denied...</p>` with `LoadingState` / `ForbiddenState`.
5. **Status everywhere:** Replace remaining `StatusBadge` imports with `StatusPill` for visual consistency.
6. **Spacing/polish:** page body uses `grid gap-6`; cards use `Card`/`CardHeader`/`CardContent`; section labels use `text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground`; buttons use shared `size`/`variant` props (no ad-hoc styling).
7. **Custom screens (Files, Search, Chat, Graph, Source Watch, Settings, Lint, Dedup, Deep Research):** these are NOT table swaps. Apply steps 1, 4, 5, 6 only (header + states + tokens + spacing). Keep the bespoke body (canvas, streaming pane, forms, result cards) but restyle it with shared primitives and consistent spacing. Never delete functionality.
8. **Preserve tests:** before editing a screen that has a co-located `*.test.tsx`, read it and keep every asserted string/role. After editing, the test must pass unchanged. If a heading string must change, update the test in the SAME task and explain why in the commit body.

**Verification for every migration task:** `npm run lint` (0 errors) AND `npm test` (all green). For visual screens with no test, additionally state in the task report that the change is type-checked and the suite is green, and that visual confirmation is pending manual review.

---

## File Map

**Phase A - Creation flows (highest priority, addresses "no UI for creating org/team/project"):**
- Modify: `src/components/layout/app-sidebar.tsx` (add "New organization" to UserChip, mount `CreateOrgDialog`)
- Modify: `src/features/spaces/create-org-dialog.tsx` (polish, `@/` imports, name->slug UX)
- Delete: `src/features/spaces/space-switcher.tsx`, `src/features/spaces/space-switcher.test.tsx` (orphaned; function absorbed by sidebar)
- Create: `src/components/layout/app-sidebar.test.tsx` (assert create-org entry reachable)
- Modify: `src/features/projects/page.tsx` (PageHeader + DataTable + polished create dialog)
- Modify: `src/features/orgs/workspace-page.tsx` (PageHeader + DataTable sections + prominent create actions + states)
- Modify: `src/features/orgs/members-page.tsx` (PageHeader + DataTable + states)
- Modify: `src/features/teams/team-page.tsx` (PageHeader + DataTable + states)
- Modify: `src/features/orgs/create-team-dialog.tsx`, `src/features/orgs/create-public-project-dialog.tsx`, `src/features/kb-access/manage-access-dialog.tsx` (`@/` imports + consistent dialog chrome)

**Phase B - Global screens:**
- Modify: `src/features/dashboard/page.tsx`, `src/features/users/page.tsx`, `src/features/api-tokens/page.tsx`, `src/features/settings/page.tsx`

**Phase C - Project-scoped tabular/list screens:**
- Modify: `src/features/projects/detail-page.tsx`, `src/features/audit/page.tsx`, `src/features/sources/page.tsx`, `src/features/reviews/page.tsx`, `src/features/dedup/page.tsx`, `src/features/lint/page.tsx`, `src/features/deep-research/page.tsx`

**Phase D - Project-scoped custom screens (chrome/token polish only):**
- Modify: `src/features/files/page.tsx`, `src/features/source-watch/page.tsx`, `src/features/search/page.tsx`, `src/features/chat/page.tsx`, `src/features/graph/page.tsx`

---

# PHASE A - Make creation flows reachable and polished

### Task A1: Surface "New organization" in the sidebar; retire orphaned SpaceSwitcher

**Files:**
- Modify: `src/components/layout/app-sidebar.tsx`
- Modify: `src/features/spaces/create-org-dialog.tsx`
- Delete: `src/features/spaces/space-switcher.tsx`
- Delete: `src/features/spaces/space-switcher.test.tsx`
- Create: `src/components/layout/app-sidebar.test.tsx`

- [ ] **Step 1: Polish `CreateOrgDialog` and switch to `@/` imports**

Rewrite `src/features/spaces/create-org-dialog.tsx` to:

```tsx
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { createOrg } from "@/features/shared/tenancy-api";

function slugify(value: string) {
  return value
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function CreateOrgDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  const [slugEdited, setSlugEdited] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: () => createOrg({ name: name.trim(), slug: (slugEdited ? slug : slugify(name)).trim() }),
    onSuccess: async (org) => {
      await queryClient.invalidateQueries({ queryKey: ["spaces"] });
      onOpenChange(false);
      navigate(`/orgs/${org.id}`);
    },
  });

  const submit = async () => {
    setErrorMessage(null);
    try {
      await mutation.mutateAsync();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create organization");
    }
  };

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>New organization</DialogTitle>
          <DialogDescription>
            Create a shared workspace for teams and public knowledge bases.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-3">
          <label className="grid gap-1.5 text-sm font-medium">
            Name
            <Input
              onChange={(event) => setName(event.target.value)}
              placeholder="Acme Research"
              value={name}
            />
          </label>
          <label className="grid gap-1.5 text-sm font-medium">
            Slug
            <Input
              onChange={(event) => {
                setSlugEdited(true);
                setSlug(event.target.value);
              }}
              placeholder={slugify(name) || "acme-research"}
              value={slugEdited ? slug : slugify(name)}
            />
          </label>
          {errorMessage ? <p className="text-sm text-destructive">{errorMessage}</p> : null}
        </div>
        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} type="button" variant="outline">
            Cancel
          </Button>
          <Button disabled={!name.trim() || mutation.isPending} onClick={submit} type="button">
            Create organization
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 2: Add the create-org entry point to the sidebar `UserChip`**

In `src/components/layout/app-sidebar.tsx`:
- Add `import { useState } from "react";` (merge into existing react import if any; currently the file imports only from `react-router-dom` and lucide).
- Add `Plus` to the lucide import: `import { ChevronLeft, Plus } from "lucide-react";`
- Add `import { CreateOrgDialog } from "@/features/spaces/create-org-dialog";`
- Replace the `UserChip` function body's `return (...)` with the version below (adds a "New organization" item and mounts the dialog as a sibling of the menu):

```tsx
function UserChip() {
  const { user } = useSession();
  const spaces = useSpacesQuery();
  const orgs = spaces.data?.orgs ?? [];
  const label = user?.username ?? "Account";
  const initial = label.slice(0, 1).toUpperCase();
  const [createOrgOpen, setCreateOrgOpen] = useState(false);

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <SidebarMenuButton className="gap-2">
              <Avatar className="size-6">
                <AvatarFallback className="bg-sidebar-primary text-[11px] text-sidebar-primary-foreground">
                  {initial}
                </AvatarFallback>
              </Avatar>
              <span className="truncate">{label}</span>
            </SidebarMenuButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="w-56" side="top">
            <DropdownMenuLabel>Spaces</DropdownMenuLabel>
            <DropdownMenuItem asChild>
              <Link to="/projects">Personal</Link>
            </DropdownMenuItem>
            {orgs.map((org) => (
              <DropdownMenuItem asChild key={org.id}>
                <Link to={`/orgs/${org.id}`}>{org.name}</Link>
              </DropdownMenuItem>
            ))}
            <DropdownMenuItem onSelect={() => setCreateOrgOpen(true)}>
              <Plus className="size-4" />
              New organization
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem asChild>
              <Link to="/settings">Settings</Link>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarMenuItem>
      <CreateOrgDialog onOpenChange={setCreateOrgOpen} open={createOrgOpen} />
    </SidebarMenu>
  );
}
```

- [ ] **Step 3: Delete the orphaned SpaceSwitcher and its test**

```bash
git rm src/features/spaces/space-switcher.tsx src/features/spaces/space-switcher.test.tsx
```

Confirm nothing imports it: `grep -r "space-switcher" src` returns nothing (the Grep tool, not raw grep). If anything still imports it, STOP and report.

- [ ] **Step 4: Write a test for the create-org entry point**

Create `src/components/layout/app-sidebar.test.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { SidebarProvider } from "@/components/ui/sidebar";

import { AppSidebar } from "./app-sidebar";

vi.mock("@/features/auth/use-session", () => ({
  useSession: () => ({ user: { username: "admin", role: "operator" } }),
}));

vi.mock("@/features/spaces/use-spaces", () => ({
  useSpacesQuery: () => ({ data: { orgs: [] }, isLoading: false }),
}));

vi.mock("@/features/projects/detail-queries", () => ({
  useProjectDetailQuery: () => ({ data: undefined, isLoading: false }),
}));

function renderSidebar() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/projects"]}>
        <SidebarProvider>
          <AppSidebar />
        </SidebarProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("AppSidebar create-org entry", () => {
  it("opens the create-organization dialog from the user menu", async () => {
    const user = userEvent.setup();
    renderSidebar();

    await user.click(screen.getByRole("button", { name: /admin/i }));
    await user.click(await screen.findByText("New organization"));

    expect(await screen.findByRole("heading", { name: "New organization" })).toBeInTheDocument();
  });
});
```

- [ ] **Step 5: Run checks**

Run: `npm run lint && npm test`
Expected: type check clean; the new `app-sidebar.test.tsx` passes; the deleted `space-switcher.test.tsx` is gone; all other suites green.

If the dropdown-to-dialog focus handoff causes the test to flake (Radix focus scope), keep the dialog mounted as a sibling (as written) and ensure `onSelect={() => setCreateOrgOpen(true)}` does NOT call `event.preventDefault()`. Do not nest the Dialog inside `DropdownMenuContent`.

- [ ] **Step 6: Commit**

```bash
git add src/components/layout/app-sidebar.tsx src/components/layout/app-sidebar.test.tsx src/features/spaces/create-org-dialog.tsx
git commit -m "feat(admin): surface create-organization flow in sidebar, retire orphaned space switcher"
```

---

### Task A2: Migrate the Projects list onto PageHeader + DataTable

**Files:**
- Modify: `src/features/projects/page.tsx`
- Test (preserve): `src/features/projects/detail-page.test.tsx` asserts a link `name: "demo-project"` and navigates from `/projects`. Keep the project name rendered as a `Link` to `/projects/:id` with the exact visible name.

- [ ] **Step 1: Read the test that covers this screen**

Read `src/features/projects/detail-page.test.tsx`. Note: it clicks `screen.findByRole("link", { name: "demo-project" })` from the `/projects` route, so the Projects table MUST render each project name as a router `Link` whose accessible name is the project name. `useProjectsQuery` returns `{ id, name, rootPath, createdAt }`.

- [ ] **Step 2: Rewrite `src/features/projects/page.tsx`**

Keep `useCreateProjectMutation`, the create Dialog, and `ProjectlessState` behavior. Swap `PageSection` -> `PageHeader`, the raw `Card`+`Table` -> `DataTable`. Full file:

```tsx
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";

import { ProjectlessState } from "@/components/layout/projectless-state";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";

import { useCreateProjectMutation } from "./mutations";
import { useProjectsQuery } from "./queries";

type ProjectRow = NonNullable<ReturnType<typeof useProjectsQuery>["data"]>[number];

export function ProjectsPage() {
  const navigate = useNavigate();
  const projects = useProjectsQuery();
  const createProject = useCreateProjectMutation();
  const [createOpen, setCreateOpen] = useState(false);
  const [name, setName] = useState("");
  const [errorMessage, setErrorMessage] = useState("");

  const projectList = projects.data ?? [];
  const hasProjects = projectList.length > 0;

  function openCreateProject() {
    setName("");
    setErrorMessage("");
    setCreateOpen(true);
  }

  function closeCreateProject() {
    setCreateOpen(false);
    setName("");
    setErrorMessage("");
  }

  async function handleCreateProject() {
    const trimmedName = name.trim();
    if (!trimmedName) {
      setErrorMessage("Name is required.");
      return;
    }
    try {
      const project = await createProject.mutateAsync({
        name: trimmedName,
        csrfToken: window.sessionStorage.getItem("knowledge.csrfToken") ?? "",
      });
      closeCreateProject();
      navigate(`/projects/${project.id}`);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create project.");
    }
  }

  const columns = useMemo<ColumnDef<ProjectRow>[]>(
    () => [
      {
        accessorKey: "name",
        header: "Name",
        cell: ({ row }) => (
          <Link
            className="font-medium text-foreground underline-offset-4 hover:underline"
            to={`/projects/${row.original.id}`}
          >
            {row.original.name}
          </Link>
        ),
      },
      {
        accessorKey: "rootPath",
        header: "Root Path",
        cell: ({ row }) => (
          <span className="font-mono text-[11px] text-muted-foreground">{row.original.rootPath}</span>
        ),
      },
      {
        accessorKey: "createdAt",
        header: "Created",
        cell: ({ row }) => (
          <span className="text-sm text-muted-foreground">{row.original.createdAt}</span>
        ),
      },
    ],
    [],
  );

  return (
    <Dialog
      onOpenChange={(open) => {
        setCreateOpen(open);
        if (!open) {
          setName("");
          setErrorMessage("");
        }
      }}
      open={createOpen}
    >
      <div className="grid gap-6">
        <PageHeader
          actions={hasProjects ? <Button onClick={openCreateProject}>Create Project</Button> : undefined}
          description="Manage registered knowledge workspaces and jump directly into project operations."
          title="Projects"
        />
        {hasProjects ? (
          <DataTable columns={columns} data={projectList} isLoading={projects.isLoading} />
        ) : (
          <ProjectlessState onCreateProject={openCreateProject} />
        )}
      </div>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create Project</DialogTitle>
          <DialogDescription>
            Register a project workspace and let the backend create the root structure.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4">
          <label className="grid gap-2 text-sm font-medium">
            Name
            <Input
              onChange={(event) => setName(event.target.value)}
              placeholder="research-notes"
              value={name}
            />
          </label>
          {errorMessage ? <p className="text-sm text-destructive">{errorMessage}</p> : null}
        </div>
        <DialogFooter>
          <Button onClick={closeCreateProject} variant="outline">
            Cancel
          </Button>
          <Button onClick={handleCreateProject}>Create Project</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 3: Run checks**

Run: `npm run lint && npm test`
Expected: clean. `detail-page.test.tsx` still finds the `demo-project` link and navigates.

- [ ] **Step 4: Commit**

```bash
git add src/features/projects/page.tsx
git commit -m "refactor(admin): migrate projects list to shared PageHeader and DataTable"
```

---

### Task A3: Migrate the Org Workspace page and make create actions prominent

**Files:**
- Modify: `src/features/orgs/workspace-page.tsx`
- Test (preserve): `src/features/orgs/workspace-page.test.tsx`

- [ ] **Step 1: Read the test**

Read `src/features/orgs/workspace-page.test.tsx` and list every asserted string/role (e.g. the org name heading text, "New public project", "New team", "Members", project/team names, "Manage access", empty-state copy). Keep all of them.

- [ ] **Step 2: Rewrite the page using PageHeader + DataTable sections**

Apply the Migration Recipe. Concrete requirements:
- Replace the raw `<header><h1>{org.name} workspace</h1>...` with `PageHeader title={`${org.name} workspace`}` and put the admin actions into `actions={...}`: a `New public project` button (admin), a `Members` link styled as `Button variant="outline"` via `asChild`, and a `New team` button. Non-admins keep just `New team`.
- Replace `if (spaces.isLoading) return <p>Loading...</p>;` with `if (spaces.isLoading) return <LoadingState rows={6} />;` (import from `@/components/shared/states`).
- Replace `if (!org) return <p>Access denied or organization not found.</p>;` with `return <ForbiddenState description="You do not have access to this organization, or it does not exist." />;` (import `ForbiddenState`).
- Replace the "Public projects" `<ul>` with a `DataTable` whose columns are `Name` (Link to `/projects/:id`) and an `actions` column rendering the admin-only `Manage access` button (keep the `openManage(project.id, null)` handler). `emptyMessage="No public projects yet."`.
- For each team `<section>`, wrap in a `Card`. Header row: `CardTitle` `Team - {team.name}` plus the `New team KB` button (when `canManageTeam && teamSpaceId`) and the `Manage` link. Body: a `DataTable` of that team's projects (Name link + admin/leader `Manage access` action), `emptyMessage="No team KBs yet."`.
- Keep ALL dialog mounts unchanged at the bottom: `CreatePublicProjectDialog` (x2), `CreateTeamDialog`, and the conditional `ManageAccessDialog`. Keep all state and handlers (`publicDialogOpen`, `teamDialogOpen`, `newKbTeamSpaceId`, `manageProjectId`, `manageTeamId`, `openManage`).
- Wrap everything in `<div className="grid gap-6">`.
- Section labels use `text-sm font-medium` (matching current) so the test's `"Public projects"`/`"Team - ..."` text assertions still resolve. If the test asserts those as `<h2>`, keep them as headings.

- [ ] **Step 3: Run checks**

Run: `npm run lint && npm test`
Expected: clean; `workspace-page.test.tsx` green.

- [ ] **Step 4: Commit**

```bash
git add src/features/orgs/workspace-page.tsx
git commit -m "refactor(admin): migrate org workspace to shared layout with prominent create actions"
```

---

### Task A4: Migrate Org Members and Team pages

**Files:**
- Modify: `src/features/orgs/members-page.tsx`
- Modify: `src/features/teams/team-page.tsx`
- Tests (preserve): `src/features/orgs/members-page.test.tsx`, `src/features/teams/team-page.test.tsx`

- [ ] **Step 1: Read both tests**

Read `members-page.test.tsx` (asserts "alice", "bob", role button `/Add member/i`, role-based button visibility) and `team-page.test.tsx` (asserts "Team - Platform", "lead", "dev", "Runbook", `/Add member/i`, `/Remove dev/i`, leader-removal absence). Preserve every assertion.

- [ ] **Step 2: Migrate `members-page.tsx`**

Apply the Migration Recipe:
- `PageHeader title={`${org.name} members`}` (keep the exact `{org.name} members` text the test asserts on, if any; otherwise the heading just needs to contain the org name).
- Replace `<p>Loading...</p>` -> `<LoadingState rows={5} />`; `<p>Access denied or organization not found.</p>` -> `<ForbiddenState />`.
- Replace the raw `<table>` with `DataTable`. Columns: `Username`, `Role` (use `StatusPill value={member.role}` for consistency OR keep plain text if a test asserts exact role text - `StatusPill` renders the raw value as its text, so `StatusPill value="org_admin"` shows `org_admin`; safe). Admin-only `Actions` column with the role `<select>` (keep `aria-label={`Role for ${member.username}`}`) and `Remove` button (keep `aria-label={`Remove ${member.username}`}`).
- Keep the admin-only "add member" controls (Input `Username`, role `<select aria-label="New member role">`, `Add member` button) in `PageHeader actions` or a `Card` below the table - keep the `Add member` accessible name.

- [ ] **Step 3: Migrate `team-page.tsx`**

Apply the Recipe similarly:
- `PageHeader title={`Team - ${team.name}`}` (keep the `Team - {name}` string) with a `Back to workspace` link (`asChild` Button `variant="ghost"`) in `actions`.
- `<p>Loading...</p>` -> `<LoadingState />`; `<p>Access denied or team not found.</p>` -> `<ForbiddenState />`.
- Members `<table>` -> `DataTable` (Username, Role via StatusPill, admin/leader Actions with `Remove` keeping `aria-label={`Remove ${member.username}`}`; leader excluded as today).
- Team KBs list: keep as `DataTable` (Name link) or a `Card` list; keep the `No team KBs yet.` empty copy and the `Runbook` item visible.
- Keep `Add member` input/button, `New team KB` input/button, and the `ManageAccessDialog` mount + `Manage access` buttons exactly.

- [ ] **Step 4: Run checks**

Run: `npm run lint && npm test`
Expected: clean; both tests green.

- [ ] **Step 5: Commit**

```bash
git add src/features/orgs/members-page.tsx src/features/teams/team-page.tsx
git commit -m "refactor(admin): migrate org members and team pages to shared layout"
```

---

### Task A5: Normalize the shared creation/access dialogs

**Files:**
- Modify: `src/features/orgs/create-team-dialog.tsx`
- Modify: `src/features/orgs/create-public-project-dialog.tsx`
- Modify: `src/features/kb-access/manage-access-dialog.tsx`

- [ ] **Step 1: Read all three files**

Read each fully to learn their current props, state, and mutations (do not change behavior or props - other pages depend on them).

- [ ] **Step 2: Apply consistent dialog chrome**

For each: convert relative `../../components/ui/*` imports to `@/components/ui/*`; ensure each uses `DialogHeader`/`DialogTitle`/`DialogDescription` and a `DialogFooter` with `Cancel` (outline) + primary action; wrap form fields in `<div className="grid gap-3">` with `label`-wrapped `Input`s; show errors as `<p className="text-sm text-destructive">`; disable the primary button while pending / when required fields are empty. Keep all existing `aria-label`s the tests rely on (`Add user`, `Grant role`, `Remove {username}`, `Add grant`).

- [ ] **Step 3: Run checks**

Run: `npm run lint && npm test`
Expected: clean; `manage-access-dialog.test.tsx` green.

- [ ] **Step 4: Commit**

```bash
git add src/features/orgs/create-team-dialog.tsx src/features/orgs/create-public-project-dialog.tsx src/features/kb-access/manage-access-dialog.tsx
git commit -m "refactor(admin): unify creation and access dialog chrome"
```

---

# PHASE B - Global screens

### Task B1: Migrate the Dashboard

**Files:**
- Modify: `src/features/dashboard/page.tsx`
- Test (preserve): `src/features/dashboard/page.test.tsx` asserts heading `name: "Dashboard"`, text `"2"`, `"openai-compatible"`, links `demo-project`/`research-notes`/`Projects`.

- [ ] **Step 1: Read the page and its test.** Note `useProjectsQuery()` (`../projects/queries`) and `useSystemSettingsQuery()` (`../settings/queries`). Recent Projects table columns: Name (Link), Root Path.

- [ ] **Step 2: Migrate.** Swap `PageSection` -> `PageHeader title="Dashboard" description="Workspace overview"`. Keep the stat `Card`s (Projects count `"2"`, Language, Default Query Limit, etc.) - restyle as a `grid gap-4 md:grid-cols-3` of `Card`s with `text-2xl font-semibold` values. Swap the Recent Projects `Table` -> `DataTable` (Name as `Link` to `/projects/:id`, Root Path mono). Keep the `EmptyState` for zero projects. Convert relative imports to `@/`.

- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: `dashboard/page.test.tsx` green.

- [ ] **Step 4: Commit**
```bash
git add src/features/dashboard/page.tsx
git commit -m "refactor(admin): migrate dashboard to shared PageHeader and DataTable"
```

---

### Task B2: Migrate the Users directory

**Files:**
- Modify: `src/features/users/page.tsx`
- Test (preserve): `src/features/users/page.test.tsx` asserts heading `name: "Users"`, text `"2 users"`, cells `"admin"`/`"editor"`, text `"member"`.

- [ ] **Step 1: Read page + test.** `useUsersQuery()` from `./queries`; rows `{ username, role }`; a "N users" stat string; role shown via `Badge`.

- [ ] **Step 2: Migrate.** `PageHeader title="Users" description="Read-only user directory for the current installation."`. Keep a `"{count} users"` summary line (the test asserts `"2 users"`) - render it as `text-sm text-muted-foreground` above the table or as the `PageHeader` description suffix; simplest is a small line above `DataTable`. Swap `Table` -> `DataTable` columns `Username` and `Role` (`StatusPill value={user.role}` so `admin`/`editor`/`member` render as text). Keep `EmptyState` for zero users. Convert imports to `@/`.

- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: `users/page.test.tsx` green (`"2 users"`, `admin`, `editor`, `member` all present).

- [ ] **Step 4: Commit**
```bash
git add src/features/users/page.tsx
git commit -m "refactor(admin): migrate users directory to shared DataTable"
```

---

### Task B3: Migrate API Tokens

**Files:**
- Modify: `src/features/api-tokens/page.tsx`
- Test (preserve): `src/features/api-tokens/page.test.tsx` asserts buttons `Mint Token`/`Revoke`/`Copy`; text `ci-bot`, `abcd1234...`, `revoked`, `plaintext-token-once`, `Copy this token now - it will not be shown again`.

- [ ] **Step 1: Read page + test.** Hooks: `useApiTokensQuery`, `useCreateApiTokenMutation`, `useRevokeApiTokenMutation` (`./queries`), `useProjectsQuery` (`../projects/queries`). Table cols: Name, Prefix, Project, Status, Last Used, Created, Actions. Already uses `@/`.

- [ ] **Step 2: Migrate.** `PageHeader title="API Tokens" description="Mint and revoke API tokens for programmatic access. Tokens carry your user identity and project memberships."`. Put the mint form (Input `aria-label="Token Name"`, scope `<select>`, `Mint Token` button) into a `Card` titled "Mint a token" (keep accessible names). Keep the one-time minted-token `Card` with `Copy`/`Dismiss` and the exact warning string. Swap the tokens `Table` -> `DataTable`: `Status` via `StatusPill` (so `revoked` shows), `Prefix` mono, `Revoke` action button (keep accessible name; only on active rows). Keep `EmptyState`.

- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: `api-tokens/page.test.tsx` green.

- [ ] **Step 4: Commit**
```bash
git add src/features/api-tokens/page.tsx
git commit -m "refactor(admin): migrate api tokens to shared layout and DataTable"
```

---

### Task B4: Polish Settings (form screen - chrome only)

**Files:**
- Modify: `src/features/settings/page.tsx`
- Test (preserve): `src/features/settings/page.test.tsx` asserts buttons `Save Settings`, `/remove configured key/i`, `/Test Search/i`; text `Key will be removed on save`, `Saved`; and exact mutation payload shapes (`providerApiKey`, `clearProviderApiKey`, `clearSearchApiKey`). DO NOT change form logic or payloads.

- [ ] **Step 1: Read page + test carefully.** This is a 472-line form with delicate API-key clear/replace logic verified by tests. Behavior must not change.

- [ ] **Step 2: Chrome-only polish (Recipe steps 1, 4, 5, 6).** Swap `PageSection` -> `PageHeader title="Settings" description="Configure the provider bridge and default system behavior for the admin workbench."`. Group the form into labeled `Card`s ("Provider", "Search", "Defaults") using `CardHeader`/`CardContent` - move existing fields under the appropriate card WITHOUT renaming fields, aria-labels, or changing state/handlers. Keep `Alert` usage for messages/errors and the web-search results `<ul>`. Convert relative imports to `@/`. Do not touch `useUpdateSystemSettingsMutation` payload construction.

- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: every `settings/page.test.tsx` assertion (including payload-shape tests) green. If any payload test fails, you changed logic - revert that part.

- [ ] **Step 4: Commit**
```bash
git add src/features/settings/page.tsx
git commit -m "refactor(admin): restyle settings into sectioned cards under shared header"
```

---

# PHASE C - Project-scoped tabular/list screens

### Task C1: Migrate the project Overview (detail-page)

**Files:**
- Modify: `src/features/projects/detail-page.tsx`
- Test (preserve): `src/features/projects/detail-page.test.tsx` - asserts heading `name: "Overview"`, card titles `Recent Sources`/`Recent Tasks`/`Recent Reviews`/`Recent Audit`, content `raw/sources/demo.md`, `Imported note.md`, `Review demo.md`, `Created project demo-project`, links `Open Files`/`Open Sources`/`Open Tasks`, project name `demo-project`. Preserve ALL of these.

- [ ] **Step 1: Read page + test.** Hooks: `useProjectDetailQuery` (`./detail-queries`), plus sources/tasks/reviews/audit/source-watch queries. Sections: 3 metric cards, Project Health, Recent Sources table, Recent Tasks table, Recent Reviews cards, Recent Audit cards, Workspace Shortcuts links.

- [ ] **Step 2: Migrate.** Swap `PageSection` -> `PageHeader title="Overview"` (keep exact `"Overview"`). Keep `RouteStatePane` loading/failed/forbidden/not_found branches. Restyle metric cards as `grid gap-4 md:grid-cols-3` with `text-2xl font-semibold` counts. Convert the "Recent Sources" and "Recent Tasks" `Table`s to `DataTable` (keep the exact card titles `Recent Sources`/`Recent Tasks` as section headings/`CardTitle`; keep `raw/sources/demo.md` and `Imported note.md` rendered). Keep Recent Reviews / Recent Audit as restyled `Card`s with their titles and content. Replace `StatusBadge` -> `StatusPill`. Keep the Workspace Shortcuts `Open Files`/`Open Sources`/`Open Tasks` links. Wrap in `grid gap-6`.

- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: `detail-page.test.tsx` (the big integration test) fully green.

- [ ] **Step 4: Commit**
```bash
git add src/features/projects/detail-page.tsx
git commit -m "refactor(admin): migrate project overview to shared header, cards, and DataTable"
```

---

### Task C2: Migrate Audit (simplest table swap)

**Files:**
- Modify: `src/features/audit/page.tsx`
- Test: none co-located, but `detail-page.test.tsx` mocks `../audit/queries` and asserts `Created project demo-project` in the Overview - the Audit page itself has no dedicated test. Verify the Audit route still renders the same fields.

- [ ] **Step 1: Read the page.** `useProjectAuditLogsQuery(projectId)` (`./queries`); columns Action, Summary; `EmptyState` "No audit records".

- [ ] **Step 2: Rewrite** swapping `PageSection` -> `PageHeader title="Audit" description="..."` (keep existing description) and `Table` -> `DataTable` with columns `Action` (mono) and `Summary`. Add a `FilterToolbar` to filter by summary/action text (client-side `useState` + `Array.filter`). Keep `EmptyState`/empty message. Use `@/` imports.

- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: clean; `detail-page.test.tsx` unaffected.

- [ ] **Step 4: Commit**
```bash
git add src/features/audit/page.tsx
git commit -m "refactor(admin): migrate audit log to shared DataTable with filter"
```

---

### Task C3: Migrate Sources (table + import tabs)

**Files:**
- Modify: `src/features/sources/page.tsx`
- Test: none co-located. `detail-page.test.tsx` mocks `../sources/queries` for the Overview only.

- [ ] **Step 1: Read the page.** Hooks: `useProjectSourcesQuery`, `useImportSourceMutation`, `useIngestSourceMutation`, `useRescanSourcesMutation`, `useDeleteSourceMutation` (`./queries`); `fileToBase64` (`../shared/api`). Tabs: Text Import / File Upload / Folder Import. Table cols: Path, Size, Actions (Ingest, Delete).

- [ ] **Step 2: Migrate.** `PageHeader title="Sources" description="..."` with `Rescan Sources` button in `actions`. Keep the `Tabs` import panel (3 tabs) inside a `Card` titled "Import sources" - keep all field labels and button names (`Import Source`, `Upload Files`, `Import Folder`). Swap the sources `Table` -> `DataTable`: `Path` via `ProjectFileLink`, `Size` right-aligned mono, Actions (`Ingest`, `Delete`). Add `FilterToolbar` over the table (filter by path). Keep `RouteStatePane` loading/failed and `EmptyState`. Use `@/` imports.

- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: clean.

- [ ] **Step 4: Commit**
```bash
git add src/features/sources/page.tsx
git commit -m "refactor(admin): migrate sources to shared header, import card, and DataTable"
```

---

### Task C4: Migrate Reviews

**Files:**
- Modify: `src/features/reviews/page.tsx`
- Test: none co-located. `detail-page.test.tsx` mocks `../reviews/queries`; keep query hook signatures.

- [ ] **Step 1: Read the page.** Hooks: `useProjectReviewsQuery(projectId, { status, itemType, limit })`, `useSweepReviewsMutation`, `useUpdateReviewMutation` (`./queries`). Card list with status/type/title/sourcePath/affectedPages/searchQueries/options; filters: status `<select>`, type `<Input>`, limit `<Input>`; `Sweep Reviews` action; per-review `Resolve`.

- [ ] **Step 2: Migrate.** `PageHeader title="Reviews" description="..."` with `Sweep Reviews` in `actions`. Move the status/type/limit filters into a `FilterToolbar` (status `Select`, type via search input, limit numeric) - keep wiring to the query args. Render reviews as a `DataTable` with columns: `Status` (`StatusPill`), `Type`, `Title`, `Source`, `Actions` (`Resolve` + any option buttons). Where a review needs its expanded metadata (affected pages, search queries, options), render it in an expandable detail `Card` or a right-hand detail pane (mirror the Tasks screen master/detail). Replace `StatusBadge` -> `StatusPill`. Keep `EmptyState`. Use `@/` imports.

- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: clean.

- [ ] **Step 4: Commit**
```bash
git add src/features/reviews/page.tsx
git commit -m "refactor(admin): migrate reviews to shared header, filter toolbar, and DataTable"
```

---

### Task C5: Polish Dedup (card screen - chrome + status)

**Files:**
- Modify: `src/features/dedup/page.tsx`
- Test (preserve): `src/features/dedup/page.test.tsx` asserts `No duplicate candidates`, `attention / attention-mechanism`, `Both describe the attention mechanism.`, `Canonical Slug` select, `Merge` button, `Not Duplicates` button, `rope, rotary-embedding` badge, `Dismissed Pairs`.

- [ ] **Step 1: Read page + test.** Hooks: `useProjectDedupQuery`, `useDetectDuplicatesMutation`, `useMergeDuplicateGroupMutation`, `useDismissDuplicateGroupMutation` (`./queries`).

- [ ] **Step 2: Chrome polish (Recipe 1,4,5,6).** Swap `PageSection` -> `PageHeader title="Dedup" description="..."` with `Detect Duplicates` in `actions`. Restyle each duplicate group as a `Card` (`CardHeader` with the slug pair, `CardContent` with the `Canonical Slug` `Select`, `Merge`, `Not Duplicates`). Keep `EmptyState` `No duplicate candidates`. Keep the `Dismissed Pairs` `Badge` section. Preserve every asserted string and accessible name.

- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: `dedup/page.test.tsx` green.

- [ ] **Step 4: Commit**
```bash
git add src/features/dedup/page.tsx
git commit -m "refactor(admin): restyle dedup groups under shared header"
```

---

### Task C6: Polish Lint (task-poll card screen - chrome + status)

**Files:**
- Modify: `src/features/lint/page.tsx`
- Test (preserve): `src/features/lint/page.test.tsx` asserts `Run Structural Lint`, `Run Semantic Lint`, `broken-link`, `wiki/concepts/attention.md`, detail/description, `Affected Pages` with links.

- [ ] **Step 1: Read page + test.** Hooks: `useCreateLintTaskMutation`, `useLintTaskDetailQuery(projectId, activeTaskId)` (`./queries`).

- [ ] **Step 2: Chrome polish.** `PageHeader title="Lint" description="..."` with `Run Structural Lint` + `Run Semantic Lint` in `actions`. Show the running task status with `StatusPill` (replace the existing `Badge`). Render issue results as restyled `Card`s keeping `broken-link`, the path `wiki/concepts/attention.md` (via `ProjectFileLink`), the description, and the `Affected Pages` links. Keep `EmptyState` `No issues found`. Preserve every asserted string + button name.

- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: `lint/page.test.tsx` green.

- [ ] **Step 4: Commit**
```bash
git add src/features/lint/page.tsx
git commit -m "refactor(admin): restyle lint results under shared header with status pills"
```

---

### Task C7: Polish Deep Research (form + filtered list - chrome + status)

**Files:**
- Modify: `src/features/deep-research/page.tsx`
- Test (preserve): `src/features/deep-research/page.test.tsx` asserts `Topic` input, `Search Queries` input, `Start Research` button, the task title, `Sources used:`, `Error:` prefix, a `ProjectFileLink` for the saved path.

- [ ] **Step 1: Read page + test.** Hooks: `useCreateDeepResearchTaskMutation` (`./queries`), `useProjectTasksQuery` (`../tasks/queries`, filtered to `taskType === "project.deep_research"`).

- [ ] **Step 2: Chrome polish.** `PageHeader title="Deep Research" description="..."`. Put the `Topic` / `Search Queries` inputs + `Start Research` button into a `Card` titled "New research" (keep labels/names). Render the filtered task list as restyled `Card`s using `StatusPill` for status, keeping `Sources used:`, the `Error:` prefix, and the `ProjectFileLink`. Keep `EmptyState`. Preserve every asserted string.

- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: `deep-research/page.test.tsx` green.

- [ ] **Step 4: Commit**
```bash
git add src/features/deep-research/page.tsx
git commit -m "refactor(admin): restyle deep research under shared header with status pills"
```

---

# PHASE D - Project-scoped custom screens (chrome/token polish only)

> These screens keep their bespoke bodies (file tree + editor, search results, streaming chat, graph canvas, watcher form). Apply Recipe steps 1, 4, 5, 6 only: shared `PageHeader`, shared loading/error/empty states, `StatusPill` for any status, consistent `grid gap-6` spacing and `Card`/section tokens. Do NOT convert their bodies to `DataTable`. Do NOT remove functionality.

### Task D1: Polish Files (tree + editor)

**Files:** Modify `src/features/files/page.tsx`. Test: none co-located.

- [ ] **Step 1: Read the page.** Two-pane: root `Select`, max-files `Input`, recursive checkbox, new-page input + `Create Page`, recursive file tree, `<pre>` preview + `WikiPageEditor`. Hooks: `useProjectFilesQuery`, `useProjectFileContentQuery`, `useSaveFileContentMutation` (`./queries`).
- [ ] **Step 2: Polish.** Swap `PageSection` -> `PageHeader title="Files" description="..."` with the root/max-files/recursive controls in `actions` or a slim toolbar `Card`. Keep the tree + preview/editor two-pane layout in `grid gap-6 xl:grid-cols-[minmax(0,360px)_minmax(0,1fr)]`. Keep `RouteStatePane` loading/failed and the two `EmptyState`s. Restyle tree rows and the `Create Page` control with shared `Button`/`Input` only. Use `@/` imports.
- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: clean.
- [ ] **Step 4: Commit**
```bash
git add src/features/files/page.tsx
git commit -m "refactor(admin): restyle files explorer chrome under shared header"
```

### Task D2: Polish Source Watch (form)

**Files:** Modify `src/features/source-watch/page.tsx`. Test: none co-located.

- [ ] **Step 1: Read the page.** Pure form: Watcher Settings card (checkboxes + inputs) + Watcher Status card. Hooks: `useProjectSourceWatchQuery`, `useUpdateProjectSourceWatchMutation`, `useScanProjectSourceWatchMutation` (`./queries`).
- [ ] **Step 2: Polish.** Swap `PageSection` -> `PageHeader title="Source Watch" description="..."` with `Scan Now` + `Save Source Watch` in `actions`. Keep the two `Card`s; tidy field layout to `grid gap-4 sm:grid-cols-2`. Replace `RouteStatePane` loading/failed as today. Show last-scan status via `StatusPill` if a status string exists. Use `@/` imports.
- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: clean.
- [ ] **Step 4: Commit**
```bash
git add src/features/source-watch/page.tsx
git commit -m "refactor(admin): restyle source watch form under shared header"
```

### Task D3: Polish Search (on-demand results)

**Files:** Modify `src/features/search/page.tsx`. Test: none co-located.

- [ ] **Step 1: Read the page.** Query input, top-k input, include-content checkbox, `Run Search`; custom result cards; `EmptyState` ready/no-matches. Hook: `useProjectSearchMutation` (`./queries`).
- [ ] **Step 2: Polish.** Swap `PageSection` -> `PageHeader title="Search" description="..."`. Put the query controls into a `Card` or `FilterToolbar`-style row with `Run Search`. Restyle result cards with shared `Card`/`Badge`, mono paths via `ProjectFileLink`, consistent spacing. Keep both `EmptyState`s. Use `@/` imports.
- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: clean.
- [ ] **Step 4: Commit**
```bash
git add src/features/search/page.tsx
git commit -m "refactor(admin): restyle search under shared header"
```

### Task D4: Polish Chat (streaming two-pane)

**Files:** Modify `src/features/chat/page.tsx`. Test: none co-located.

- [ ] **Step 1: Read the page.** Custom two-pane: conversation aside (`New conversation`, list with `Rename`/`Delete`) + message pane (streamed messages, textarea, `Send`). Hooks: `useConversationsQuery`, `useConversationMessagesQuery`, `useCreateConversationMutation`, `useRenameConversationMutation`, `useDeleteConversationMutation`, `streamChatMessage` (`./stream`).
- [ ] **Step 2: Polish (chrome only - do NOT touch streaming logic).** Add a `PageHeader title="Chat" description="..."` above the two-pane (or keep the embedded layout but adopt shared tokens). Replace raw buttons/textarea with shared `Button`/`Textarea`/`Input` from `@/components/ui/*`. Style the conversation aside as a bordered `Card`-like rail; message bubbles use consistent `rounded-md border` tokens and `bg-muted` for assistant / accent for user. Keep `streamingText`/`pendingUserText`/`error` handling unchanged. Use `@/` imports.
- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: clean.
- [ ] **Step 4: Commit**
```bash
git add src/features/chat/page.tsx
git commit -m "refactor(admin): restyle chat shell with shared primitives"
```

### Task D5: Polish Graph (canvas + controls)

**Files:** Modify `src/features/graph/page.tsx`. Test (preserve): `src/features/graph/page.test.tsx` asserts heading `Graph`, `Search graph` input, node labels (`Alpha`/`Beta`), `Hide structural`, `Insights`, `Hide this node`, `Show`, and a `data-highlighted` attribute.

- [ ] **Step 1: Read page + test.** Hook: `useProjectGraphQuery` (`./queries`); custom `GraphCanvas`, legend, insights, node-detail panels; toolbar of toggle buttons + search input.
- [ ] **Step 2: Polish (chrome only - keep canvas + filtering).** Swap `PageSection` -> `PageHeader title="Graph" description="..."` (keep heading text `Graph`). Move the toggle buttons (`Hide structural`, `Hide isolated`, `Community colors`, `Insights`) into `PageHeader actions` or a toolbar `Card`; keep the `Search graph` input (preserve its accessible name) - a `FilterToolbar` is ideal. Keep `RouteStatePane` loading/failed/forbidden/not_found. Restyle the node-detail / insights / hidden-nodes panels as shared `Card`s. Preserve `Hide this node`, `Show`, `data-highlighted`, and node labels. Use `@/` imports.
- [ ] **Step 3: Run `npm run lint && npm test`.** Expected: `graph/page.test.tsx` green.
- [ ] **Step 4: Commit**
```bash
git add src/features/graph/page.tsx
git commit -m "refactor(admin): restyle graph chrome under shared header"
```

---

## Final verification (after all tasks)

- [ ] Run `npm run lint && npm test` - entire suite green, no type errors.
- [ ] Rebuild and smoke-test in Docker: `docker compose up -d --build admin` then load the app and manually click through: open the user menu -> New organization (dialog opens, create navigates to the org); org workspace -> New team / New public project; projects -> Create Project; spot-check 4-5 migrated screens for consistent header/spacing/status styling and no old `PageSection` chrome remaining.
- [ ] `grep` for leftover old patterns (using the Grep tool): no remaining imports of `@/components/layout/page-section` (`PageSection`) in `src/features/**` except where intentionally retained; no remaining `StatusBadge` imports; no `space-switcher` references.
- [ ] Use superpowers:finishing-a-development-branch to complete the work.

---

## Notes on scope and sequencing

- **Phase A is the highest priority** - it directly fixes the user's complaint that org/team/project creation has no surfaced UI. It is independently shippable.
- Phases B-D are pure screen migrations and can ship incrementally; each task leaves the suite green.
- **Chat and Graph are deliberately chrome-only** - converting their bespoke bodies to tables would destroy functionality. Polish means consistent header/spacing/tokens, not a table.
- If any task's test assertions force a heading/string change, update the test IN THE SAME task and justify it in the commit body (per Recipe step 8).
