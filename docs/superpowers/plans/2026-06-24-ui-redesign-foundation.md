# Admin UI Redesign — Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the shared design-system layer for `apps/admin` — new color/type tokens, official Radix-based shadcn primitives, a route-aware unified left sidebar, and reusable presentation components (PageHeader, StatusPill, DataTable, FilterToolbar, states) — then migrate the Tasks screen onto it as the reference implementation.

**Architecture:** Three layers change in order: (1) design tokens in `src/index.css`; (2) the official shadcn primitive kit in `src/components/ui/` (migrated via the shadcn CLI, retuned to the new tokens); (3) the app shell + unified sidebar + shared components in `src/components/layout/` and a new `src/components/shared/`. Feature screens are migrated afterward — this plan migrates only **Tasks** to prove the layer; the remaining screens are handled by follow-up per-group plans.

**Tech Stack:** React 19, React Router 7, TanStack Query 5, Tailwind CSS v4 (`@theme inline` in `index.css`), official Radix-based shadcn/ui, `@tanstack/react-table`, `class-variance-authority`, `lucide-react`, vitest 4 + Testing Library + jsdom.

---

## Scope & sequencing note

The design spec (`docs/superpowers/specs/2026-06-24-ui-redesign-design.md`) lists migrating **every** primitive (including Select/Dialog/Tabs/Tooltip/Sheet) in the foundation. This plan deliberately migrates only the primitives the new shell + shared components + Tasks screen require:

- **Migrated now (CLI):** `sidebar`, `dropdown-menu`, `avatar`, `card`, `table`, `badge`, `button`, `input`, `separator`, `sheet`, `skeleton`, `tooltip`. (The official `sidebar` pulls in `sheet`/`tooltip`/`separator`/`skeleton`/`input`/`button` + a `use-mobile` hook as hard dependencies, so they come along regardless. `sheet` and `tooltip` have **zero** consumers today, so migrating them is risk-free.)
- **Deferred to per-group plans:** `select` (3 consumers: dedup, reviews, files — each reworked in its group plan), `dialog` (5 controlled consumers), `tabs` (1 consumer: sources), plus `textarea`, `alert`, `scroll-area`, `form`. The spec explicitly allows this: "Every consuming page must be updated as part of the migration (sequenced in the plans)" and "Remove legacy `@layer components` ... as their consumers migrate — not before."

Each task below leaves `npm run lint` (tsc) and `npm test` green after its commit.

---

## File Structure

**Created:**
- `apps/admin/components.json` — official shadcn config (Tailwind v4, `@` alias, base color slate).
- `apps/admin/src/components/ui/sidebar.tsx`, `dropdown-menu.tsx`, `avatar.tsx` — new official primitives (CLI-generated).
- `apps/admin/src/hooks/use-mobile.ts` — CLI-generated (sidebar dependency).
- `apps/admin/src/components/layout/app-sidebar.tsx` — route-aware unified sidebar.
- `apps/admin/src/components/shared/page-header.tsx` — standard content-page header.
- `apps/admin/src/components/shared/status-pill.tsx` — status pill (5 variants) + tests.
- `apps/admin/src/components/shared/data-table.tsx` — generic `@tanstack/react-table` table.
- `apps/admin/src/components/shared/filter-toolbar.tsx` — search + filter toolbar.
- `apps/admin/src/components/shared/states.tsx` — `LoadingState` / `ErrorState` / `ForbiddenState`.
- Test files alongside each new shared component + `app-sidebar.test.tsx`.

**Modified (CLI-overwritten then retuned):** `src/components/ui/{card,table,badge,button,input,separator,sheet,skeleton,tooltip}.tsx`.

**Modified (hand-edited):**
- `apps/admin/package.json` — add `@tanstack/react-table`, `@fontsource-variable/geist-mono` (+ Radix deps auto-added by CLI).
- `apps/admin/src/index.css` — token rewrite, mono font, flat background, 6px radius, sidebar tokens.
- `apps/admin/src/test/setup.ts` — add `matchMedia` + pointer polyfills for Radix/jsdom.
- `apps/admin/src/lib/route-meta.ts` — replace `systemRoutes`/`projectRoutes` with `globalNav` + `projectNavGroups`.
- `apps/admin/src/components/layout/app-shell.tsx` — `SidebarProvider` + `AppSidebar` + `SidebarInset` + `Outlet`.
- `apps/admin/src/components/layout/project-workspace-layout.tsx` — drop `ProjectHeader` + `ProjectNav`; render `Outlet` only.
- `apps/admin/src/components/layout/app-shell.test.tsx`, `src/app/router.test.tsx` — assert sidebar links instead of tabs/banner.
- `apps/admin/src/features/tasks/page.tsx` + `page.test.tsx` — migrate to PageHeader + DataTable + StatusPill.

**Deleted:**
- `apps/admin/src/styles.css` (dead — `main.tsx` imports only `index.css`).
- `apps/admin/src/components/layout/system-sidebar.tsx`, `top-bar.tsx`.
- `apps/admin/src/features/projects/project-nav.tsx`.

> `apps/admin/src/components/layout/project-header.tsx` is left in place (no longer rendered by the layout); the follow-up Overview plan folds it into the Overview hero.

---

## Task 1: Add dependencies + shadcn config

**Files:**
- Modify: `apps/admin/package.json`
- Create: `apps/admin/components.json`

- [ ] **Step 1: Install the two non-CLI runtime deps**

Run (from `apps/admin/`):
```bash
npm install @tanstack/react-table @fontsource-variable/geist-mono
```
Expected: `package.json` gains `@tanstack/react-table` and `@fontsource-variable/geist-mono` under `dependencies`; install succeeds.

- [ ] **Step 2: Create `components.json`**

Create `apps/admin/components.json`:
```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "",
    "css": "src/index.css",
    "baseColor": "slate",
    "cssVariables": true,
    "prefix": ""
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"
  },
  "iconLibrary": "lucide"
}
```

- [ ] **Step 3: Verify the app still builds and tests pass**

Run: `npm run lint && npm test`
Expected: tsc reports no errors; all existing tests pass (nothing references the new deps yet).

- [ ] **Step 4: Commit**

```bash
git add apps/admin/package.json apps/admin/package-lock.json apps/admin/components.json
git commit -m "build(admin): add react-table + geist-mono and shadcn config"
```

---

## Task 2: Rewrite design tokens in `index.css`

**Files:**
- Modify: `apps/admin/src/index.css`

- [ ] **Step 1: Replace the token + base layers with the new palette**

Replace the entire contents of `apps/admin/src/index.css` with:
```css
@import "tailwindcss";
@import "tw-animate-css";
@import "@fontsource-variable/geist";
@import "@fontsource-variable/geist-mono";

@theme inline {
  --font-sans: "Geist Variable", sans-serif;
  --font-heading: var(--font-sans);
  --font-mono: "Geist Mono Variable", ui-monospace, "SFMono-Regular", monospace;
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-card: var(--card);
  --color-card-foreground: var(--card-foreground);
  --color-popover: var(--popover);
  --color-popover-foreground: var(--popover-foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-secondary: var(--secondary);
  --color-secondary-foreground: var(--secondary-foreground);
  --color-muted: var(--muted);
  --color-muted-foreground: var(--muted-foreground);
  --color-accent: var(--accent);
  --color-accent-foreground: var(--accent-foreground);
  --color-destructive: var(--destructive);
  --color-destructive-foreground: var(--destructive-foreground);
  --color-border: var(--border);
  --color-input: var(--input);
  --color-ring: var(--ring);
  --color-sidebar: var(--sidebar);
  --color-sidebar-foreground: var(--sidebar-foreground);
  --color-sidebar-primary: var(--sidebar-primary);
  --color-sidebar-primary-foreground: var(--sidebar-primary-foreground);
  --color-sidebar-accent: var(--sidebar-accent);
  --color-sidebar-accent-foreground: var(--sidebar-accent-foreground);
  --color-sidebar-border: var(--sidebar-border);
  --color-sidebar-ring: var(--sidebar-ring);
  --radius-sm: calc(var(--radius) - 4px);
  --radius-md: calc(var(--radius) - 2px);
  --radius-lg: var(--radius);
  --radius-xl: calc(var(--radius) + 4px);
}

:root {
  --background: #fbfcfd;
  --foreground: #0d1117;
  --card: #ffffff;
  --card-foreground: #0d1117;
  --popover: #ffffff;
  --popover-foreground: #0d1117;
  --primary: #1f6feb;
  --primary-foreground: #ffffff;
  --secondary: #f6f7f8;
  --secondary-foreground: #24292f;
  --muted: #f6f7f8;
  --muted-foreground: #57606a;
  --accent: #eaeef2;
  --accent-foreground: #0d1117;
  --destructive: #c21f2e;
  --destructive-foreground: #ffffff;
  --border: #d0d7de;
  --input: #d0d7de;
  --ring: #1f6feb;
  --radius: 0.375rem;
  --sidebar: #f6f7f8;
  --sidebar-foreground: #24292f;
  --sidebar-primary: #1f6feb;
  --sidebar-primary-foreground: #ffffff;
  --sidebar-accent: #eaeef2;
  --sidebar-accent-foreground: #0d1117;
  --sidebar-border: #e1e4e8;
  --sidebar-ring: #1f6feb;
}

@layer base {
  * {
    @apply border-border outline-ring/50;
  }

  html,
  body,
  #root {
    min-height: 100%;
  }

  html {
    font-family: var(--font-sans);
  }

  body {
    @apply bg-background text-foreground antialiased;
    margin: 0;
  }
}

@layer components {
  .page {
    @apply min-h-screen grid place-items-center px-6 py-10;
  }

  .shell {
    @apply min-h-screen bg-transparent;
  }

  .sidebar {
    @apply flex flex-col gap-2;
  }

  .content {
    @apply flex-1;
  }

  .stack {
    @apply grid gap-4;
  }

  .stack.compact {
    @apply gap-2;
  }

  .stats {
    @apply flex flex-wrap gap-3 text-sm text-muted-foreground;
  }

  .subnav {
    @apply flex flex-wrap gap-2;
  }

  .panel {
    @apply max-w-5xl;
  }

  .results-list {
    @apply grid gap-4 list-none p-0 m-0;
  }

  .checkbox-row {
    @apply flex items-center gap-3;
  }

  .action-row {
    @apply flex flex-wrap gap-3;
  }

  .files-layout {
    @apply grid gap-6 xl:grid-cols-[320px_minmax(0,1fr)];
  }

  .graph-layout {
    @apply grid gap-6 xl:grid-cols-[minmax(0,1.3fr)_360px];
  }

  .tree-list {
    @apply grid gap-2 list-none p-0 m-0;
  }

  .tree-list.nested {
    @apply ml-4 mt-2 border-l border-border pl-4;
  }

  .tree-item {
    @apply grid gap-2;
  }

  .tree-label {
    @apply text-sm text-muted-foreground;
  }

  .tree-button {
    @apply justify-start;
  }

  .preview-pane {
    @apply rounded-md border bg-muted/50 p-4 text-sm whitespace-pre-wrap break-words overflow-auto;
  }

  .ghost-button {
    @apply inline-flex cursor-pointer items-center rounded-md bg-transparent px-2 py-1 text-sm text-primary hover:bg-accent hover:text-accent-foreground;
  }

  .inline-link {
    @apply text-primary underline-offset-4 hover:underline break-all;
  }
}
```

- [ ] **Step 2: Verify the dev server renders with the new palette**

Run: `npm run dev`, open the app, confirm the background is flat off-white (`#fbfcfd`, no gradient) and primary buttons are blue. Stop the server.

- [ ] **Step 3: Verify lint + tests**

Run: `npm run lint && npm test`
Expected: no tsc errors; all tests pass (CSS-only change).

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/index.css
git commit -m "style(admin): recolor tokens to blue/slate technical console, flat bg, mono font"
```

---

## Task 3: Add Radix/jsdom test polyfills

**Files:**
- Modify: `apps/admin/src/test/setup.ts`

Radix primitives used by the new kit need browser APIs jsdom lacks: `window.matchMedia` (shadcn `use-mobile` hook, called by `SidebarProvider`) and pointer/scroll APIs (Radix `DropdownMenu` interactions under `user-event`).

- [ ] **Step 1: Replace `setup.ts` with the polyfilled version**

Replace the entire contents of `apps/admin/src/test/setup.ts` with:
```ts
import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

if (!window.matchMedia) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

if (!Element.prototype.hasPointerCapture) {
  Element.prototype.hasPointerCapture = vi.fn(() => false);
}
if (!Element.prototype.setPointerCapture) {
  Element.prototype.setPointerCapture = vi.fn();
}
if (!Element.prototype.releasePointerCapture) {
  Element.prototype.releasePointerCapture = vi.fn();
}
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = vi.fn();
}

afterEach(() => {
  cleanup();
});
```

- [ ] **Step 2: Verify lint + tests**

Run: `npm run lint && npm test`
Expected: no tsc errors; all tests still pass (polyfills are additive).

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/test/setup.ts
git commit -m "test(admin): polyfill matchMedia and pointer APIs for radix/jsdom"
```

---

## Task 4: Migrate primitives to official Radix shadcn (CLI) + retune Card

**Files:**
- Create: `apps/admin/src/components/ui/sidebar.tsx`, `dropdown-menu.tsx`, `avatar.tsx`, `apps/admin/src/hooks/use-mobile.ts`
- Overwrite: `apps/admin/src/components/ui/{card,table,badge,button,input,separator,sheet,skeleton,tooltip}.tsx`
- Modify (retune): `apps/admin/src/components/ui/card.tsx`

- [ ] **Step 1: Generate/overwrite the primitives via the shadcn CLI**

Run (from `apps/admin/`):
```bash
npx shadcn@latest add sidebar dropdown-menu avatar card table badge button input separator sheet skeleton tooltip --overwrite --yes
```
Expected: writes `src/components/ui/{sidebar,dropdown-menu,avatar,card,table,badge,button,input,separator,sheet,skeleton,tooltip}.tsx`, creates `src/hooks/use-mobile.ts`, and auto-installs the Radix deps (`@radix-ui/react-dropdown-menu`, `@radix-ui/react-avatar`, `@radix-ui/react-separator`, `@radix-ui/react-tooltip`, `@radix-ui/react-dialog` [for sheet], `@radix-ui/react-slot`) into `package.json`.

- [ ] **Step 2: Retune `CardTitle` to render an `<h3>`**

The official `card.tsx` renders `CardTitle` as a `<div>`, which breaks the 9 existing tests (and `RouteStatePane`/`EmptyState`) that query `getByRole("heading", ...)`. In `apps/admin/src/components/ui/card.tsx`, change the `CardTitle` component so it renders an `h3` instead of a `div` — keep the generated `className` string and `data-slot` exactly as produced by the CLI, only swap the element and prop type:
```tsx
function CardTitle({ className, ...props }: React.ComponentProps<"h3">) {
  return (
    <h3
      data-slot="card-title"
      className={cn("leading-none font-semibold", className)}
      {...props}
    />
  );
}
```

- [ ] **Step 3: Verify lint + full test suite**

Run: `npm run lint && npm test`
Expected: no tsc errors; ALL tests pass. The `Select`/`Dialog`/`Tabs` primitives are untouched (still hand-rolled), so their consumers and tests are unaffected. If any `getByRole("heading", ...)` test fails, confirm `CardTitle` is an `<h3>` (Step 2).

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/components/ui apps/admin/src/hooks/use-mobile.ts apps/admin/package.json apps/admin/package-lock.json
git commit -m "feat(admin): migrate base primitives to official radix shadcn, keep CardTitle as h3"
```

---

## Task 5: Restructure navigation metadata

**Files:**
- Modify: `apps/admin/src/lib/route-meta.ts`

`systemRoutes`/`projectRoutes` are consumed only by `system-sidebar.tsx` and `project-nav.tsx`, both removed in Task 7. Replace them with a flat `globalNav` and a grouped `projectNavGroups`. (Tasks 6 and 7 introduce the consumers of these; this task only changes the data and removes the old exports together with their consumers is done in Task 7 — but `route-meta.ts` has no other importers, so swapping exports here is safe and `npm test` stays green because the removed files are deleted in the same plan. To keep THIS task green, the old `system-sidebar.tsx`/`project-nav.tsx` still import the old names — so this task ADDS the new exports and KEEPS the old ones; Task 7 removes the old exports when it deletes their consumers.)

- [ ] **Step 1: Add the new nav structures (keep the old exports for now)**

Replace the entire contents of `apps/admin/src/lib/route-meta.ts` with:
```ts
export const systemRoutes = [
  { to: "/", label: "Dashboard" },
  { to: "/projects", label: "Projects" },
  { to: "/users", label: "Users" },
  { to: "/api-tokens", label: "API Tokens" },
  { to: "/settings", label: "Settings" },
] as const;

export const projectRoutes = [
  { suffix: "", label: "Overview" },
  { suffix: "/files", label: "Files" },
  { suffix: "/sources", label: "Sources" },
  { suffix: "/source-watch", label: "Source Watch" },
  { suffix: "/search", label: "Search" },
  { suffix: "/chat", label: "Chat" },
  { suffix: "/lint", label: "Lint" },
  { suffix: "/graph", label: "Graph" },
  { suffix: "/tasks", label: "Tasks" },
  { suffix: "/reviews", label: "Reviews" },
  { suffix: "/dedup", label: "Dedup" },
  { suffix: "/deep-research", label: "Deep Research" },
  { suffix: "/audit", label: "Audit" },
] as const;

export const globalNav = [
  { to: "/", label: "Dashboard" },
  { to: "/projects", label: "Projects" },
  { to: "/users", label: "Users" },
  { to: "/api-tokens", label: "API Tokens" },
  { to: "/settings", label: "Settings" },
] as const;

export type ProjectNavItem = { suffix: string; label: string };
export type ProjectNavGroup = { label: string | null; items: ProjectNavItem[] };

export const projectNavGroups: ProjectNavGroup[] = [
  { label: null, items: [{ suffix: "", label: "Overview" }] },
  {
    label: "Content",
    items: [
      { suffix: "/files", label: "Files" },
      { suffix: "/sources", label: "Sources" },
      { suffix: "/source-watch", label: "Source Watch" },
    ],
  },
  {
    label: "Explore",
    items: [
      { suffix: "/search", label: "Search" },
      { suffix: "/chat", label: "Chat" },
      { suffix: "/graph", label: "Graph" },
      { suffix: "/deep-research", label: "Deep Research" },
    ],
  },
  {
    label: "Quality",
    items: [
      { suffix: "/lint", label: "Lint" },
      { suffix: "/reviews", label: "Reviews" },
      { suffix: "/dedup", label: "Dedup" },
    ],
  },
  {
    label: "System",
    items: [
      { suffix: "/tasks", label: "Tasks" },
      { suffix: "/audit", label: "Audit" },
    ],
  },
];
```

- [ ] **Step 2: Verify lint + tests**

Run: `npm run lint && npm test`
Expected: no tsc errors; all tests pass (old exports retained, new exports added).

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/lib/route-meta.ts
git commit -m "feat(admin): add grouped project nav + global nav metadata"
```

---

## Task 6: Build the route-aware `AppSidebar`

**Files:**
- Create: `apps/admin/src/components/layout/app-sidebar.tsx`
- Test: `apps/admin/src/components/layout/app-sidebar.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `apps/admin/src/components/layout/app-sidebar.test.tsx`:
```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { SidebarProvider } from "@/components/ui/sidebar";

import { AppSidebar } from "./app-sidebar";

const mockUseSession = vi.fn();
const mockUseProjectDetailQuery = vi.fn();
const mockUseSpacesQuery = vi.fn();

vi.mock("@/features/auth/use-session", () => ({
  useSession: () => mockUseSession(),
}));
vi.mock("@/features/projects/detail-queries", () => ({
  useProjectDetailQuery: (...args: unknown[]) => mockUseProjectDetailQuery(...args),
}));
vi.mock("@/features/spaces/use-spaces", () => ({
  useSpacesQuery: () => mockUseSpacesQuery(),
}));

function renderSidebar(path: string) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[path]}>
        <SidebarProvider>
          <AppSidebar />
        </SidebarProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("AppSidebar", () => {
  it("renders global navigation outside a project", () => {
    mockUseSession.mockReturnValue({ user: { id: "u1", username: "admin", role: "admin" }, isLoading: false });
    mockUseProjectDetailQuery.mockReturnValue({ data: null, error: null, isLoading: false });
    mockUseSpacesQuery.mockReturnValue({ data: { orgs: [] }, isLoading: false });

    renderSidebar("/");

    expect(screen.getByRole("link", { name: "Dashboard" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Projects" })).toBeInTheDocument();
    expect(screen.getByText("admin")).toBeInTheDocument();
  });

  it("renders grouped project navigation inside a project", () => {
    mockUseSession.mockReturnValue({ user: { id: "u1", username: "admin", role: "admin" }, isLoading: false });
    mockUseProjectDetailQuery.mockReturnValue({
      data: { project: { id: "project-1", name: "demo-project", rootPath: "E:/demo" } },
      error: null,
      isLoading: false,
    });
    mockUseSpacesQuery.mockReturnValue({ data: { orgs: [] }, isLoading: false });

    renderSidebar("/projects/project-1/tasks");

    expect(screen.getByText("demo-project")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Files" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Tasks" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /all projects/i })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm test -- app-sidebar`
Expected: FAIL — `AppSidebar` is not defined / module not found.

- [ ] **Step 3: Implement `AppSidebar`**

Create `apps/admin/src/components/layout/app-sidebar.tsx`:
```tsx
import { ChevronLeft } from "lucide-react";
import { Link, NavLink, useMatch } from "react-router-dom";

import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
import { useSession } from "@/features/auth/use-session";
import { useProjectDetailQuery } from "@/features/projects/detail-queries";
import { useSpacesQuery } from "@/features/spaces/use-spaces";
import { globalNav, projectNavGroups } from "@/lib/route-meta";
import { cn } from "@/lib/utils";

function activeNavClass(isActive: boolean) {
  return cn(
    isActive &&
      "bg-sidebar-primary text-sidebar-primary-foreground hover:bg-sidebar-primary hover:text-sidebar-primary-foreground",
  );
}

export function AppSidebar() {
  const projectMatch = useMatch("/projects/:projectId/*") ?? useMatch("/projects/:projectId");
  const projectId = projectMatch?.params.projectId ?? "";
  const isProjectContext = Boolean(projectId);

  return (
    <Sidebar>
      {isProjectContext ? (
        <ProjectSidebarHeader projectId={projectId} />
      ) : (
        <SidebarHeader>
          <div className="px-2 py-1.5">
            <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
              Knowledge
            </p>
            <p className="text-sm font-semibold text-foreground">Admin Console</p>
          </div>
        </SidebarHeader>
      )}

      <SidebarContent>
        {isProjectContext ? (
          projectNavGroups.map((group, index) => (
            <SidebarGroup key={group.label ?? `group-${index}`}>
              {group.label ? <SidebarGroupLabel>{group.label}</SidebarGroupLabel> : null}
              <SidebarMenu>
                {group.items.map((item) => {
                  const to = `/projects/${projectId}${item.suffix}`;
                  return (
                    <SidebarMenuItem key={to}>
                      <SidebarMenuButton asChild>
                        <NavLink
                          end={item.suffix === ""}
                          to={to}
                          className={({ isActive }) => activeNavClass(isActive)}
                        >
                          {item.label}
                        </NavLink>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  );
                })}
              </SidebarMenu>
            </SidebarGroup>
          ))
        ) : (
          <SidebarGroup>
            <SidebarMenu>
              {globalNav.map((item) => (
                <SidebarMenuItem key={item.to}>
                  <SidebarMenuButton asChild>
                    <NavLink
                      end={item.to === "/"}
                      to={item.to}
                      className={({ isActive }) => activeNavClass(isActive)}
                    >
                      {item.label}
                    </NavLink>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroup>
        )}
      </SidebarContent>

      <SidebarFooter>
        <UserChip />
      </SidebarFooter>
    </Sidebar>
  );
}

function ProjectSidebarHeader({ projectId }: { projectId: string }) {
  const project = useProjectDetailQuery(projectId);
  const detail = project.data?.project;
  return (
    <SidebarHeader className="gap-2">
      <Link
        to="/projects"
        className="flex items-center gap-1 px-2 text-[11px] font-semibold text-muted-foreground hover:text-foreground"
      >
        <ChevronLeft className="size-3" />
        All projects
      </Link>
      <div className="px-2">
        <p className="text-sm font-semibold text-foreground">{detail?.name ?? "Project"}</p>
        <p className="font-mono text-[10px] text-muted-foreground">{projectId}</p>
      </div>
    </SidebarHeader>
  );
}

function UserChip() {
  const { user } = useSession();
  const spaces = useSpacesQuery();
  const orgs = spaces.data?.orgs ?? [];
  const label = user?.username ?? "Account";
  const initial = label.slice(0, 1).toUpperCase();

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
          <DropdownMenuContent align="start" side="top" className="w-48">
            <DropdownMenuLabel>Spaces</DropdownMenuLabel>
            <DropdownMenuItem asChild>
              <Link to="/projects">Personal</Link>
            </DropdownMenuItem>
            {orgs.map((org) => (
              <DropdownMenuItem key={org.id} asChild>
                <Link to={`/orgs/${org.id}`}>{org.name}</Link>
              </DropdownMenuItem>
            ))}
            <DropdownMenuSeparator />
            <DropdownMenuItem asChild>
              <Link to="/settings">Settings</Link>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm test -- app-sidebar`
Expected: PASS (both contexts render the expected links/text).

- [ ] **Step 5: Verify lint + full suite**

Run: `npm run lint && npm test`
Expected: no tsc errors; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/components/layout/app-sidebar.tsx apps/admin/src/components/layout/app-sidebar.test.tsx
git commit -m "feat(admin): add route-aware unified app sidebar"
```

---

## Task 7: Swap the app shell and remove the old nav chrome

**Files:**
- Modify: `apps/admin/src/components/layout/app-shell.tsx`, `project-workspace-layout.tsx`, `app-shell.test.tsx`
- Modify: `apps/admin/src/app/router.test.tsx`
- Modify: `apps/admin/src/lib/route-meta.ts` (drop old exports)
- Delete: `apps/admin/src/components/layout/system-sidebar.tsx`, `top-bar.tsx`, `apps/admin/src/features/projects/project-nav.tsx`

- [ ] **Step 1: Rework `app-shell.tsx`**

Replace the entire contents of `apps/admin/src/components/layout/app-shell.tsx` with:
```tsx
import { Outlet } from "react-router-dom";

import { SidebarInset, SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar";

import { AppSidebar } from "./app-sidebar";

export function AppShell() {
  return (
    <SidebarProvider>
      <AppSidebar />
      <SidebarInset>
        <div className="flex h-12 items-center gap-2 border-b border-border px-4 lg:hidden">
          <SidebarTrigger />
        </div>
        <main className="min-w-0 flex-1 p-4 lg:p-6">
          <Outlet />
        </main>
      </SidebarInset>
    </SidebarProvider>
  );
}
```

- [ ] **Step 2: Rework `project-workspace-layout.tsx` to render only the Outlet**

Replace the entire contents of `apps/admin/src/components/layout/project-workspace-layout.tsx` with:
```tsx
import { Outlet, useParams } from "react-router-dom";

import { normalizeAppError } from "@/lib/app-error";
import { useProjectDetailQuery } from "@/features/projects/detail-queries";

import { RouteStatePane } from "./route-state-pane";

export function ProjectWorkspaceLayout() {
  const { projectId = "" } = useParams();
  const project = useProjectDetailQuery(projectId);

  if (project.isLoading) {
    return <RouteStatePane description="Loading project context." state="loading" title="Project" />;
  }

  if (project.error) {
    const normalized = normalizeAppError(project.error);
    if (normalized.kind === "forbidden") {
      return (
        <RouteStatePane
          description="You do not have access to this project."
          state="forbidden"
          title="Access denied"
        />
      );
    }
    if (normalized.kind === "not_found") {
      return (
        <RouteStatePane
          description="The requested project no longer exists."
          state="not_found"
          title="Project not found"
        />
      );
    }
    return (
      <RouteStatePane description={normalized.message} state="failed" title="Project unavailable" />
    );
  }

  const detail = project.data?.project;
  if (!detail) {
    return (
      <RouteStatePane
        description="Project details are unavailable."
        state="not_found"
        title="Project not found"
      />
    );
  }

  return <Outlet />;
}
```

- [ ] **Step 3: Delete the obsolete nav files**

```bash
git rm apps/admin/src/components/layout/system-sidebar.tsx apps/admin/src/components/layout/top-bar.tsx apps/admin/src/features/projects/project-nav.tsx
```

- [ ] **Step 4: Drop the obsolete exports from `route-meta.ts`**

Replace the entire contents of `apps/admin/src/lib/route-meta.ts` with (removing `systemRoutes` and `projectRoutes`, now unused):
```ts
export const globalNav = [
  { to: "/", label: "Dashboard" },
  { to: "/projects", label: "Projects" },
  { to: "/users", label: "Users" },
  { to: "/api-tokens", label: "API Tokens" },
  { to: "/settings", label: "Settings" },
] as const;

export type ProjectNavItem = { suffix: string; label: string };
export type ProjectNavGroup = { label: string | null; items: ProjectNavItem[] };

export const projectNavGroups: ProjectNavGroup[] = [
  { label: null, items: [{ suffix: "", label: "Overview" }] },
  {
    label: "Content",
    items: [
      { suffix: "/files", label: "Files" },
      { suffix: "/sources", label: "Sources" },
      { suffix: "/source-watch", label: "Source Watch" },
    ],
  },
  {
    label: "Explore",
    items: [
      { suffix: "/search", label: "Search" },
      { suffix: "/chat", label: "Chat" },
      { suffix: "/graph", label: "Graph" },
      { suffix: "/deep-research", label: "Deep Research" },
    ],
  },
  {
    label: "Quality",
    items: [
      { suffix: "/lint", label: "Lint" },
      { suffix: "/reviews", label: "Reviews" },
      { suffix: "/dedup", label: "Dedup" },
    ],
  },
  {
    label: "System",
    items: [
      { suffix: "/tasks", label: "Tasks" },
      { suffix: "/audit", label: "Audit" },
    ],
  },
];
```

- [ ] **Step 5: Update `app-shell.test.tsx` to assert sidebar links**

Replace the entire contents of `apps/admin/src/components/layout/app-shell.test.tsx` with:
```tsx
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";

import { AppShell } from "./app-shell";

describe("AppShell", () => {
  it("renders the global sidebar navigation", () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={client}>
        <MemoryRouter initialEntries={["/"]}>
          <AppShell />
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(screen.getByRole("link", { name: "Dashboard" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Projects" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Settings" })).toBeInTheDocument();
  });
});
```

- [ ] **Step 6: Update `router.test.tsx` to assert sidebar links instead of tabs**

In `apps/admin/src/app/router.test.tsx`, replace the body of the second test (`"shows project workspace chrome on /projects/:projectId routes"`) assertions block — specifically replace lines 134-142 (the `findByRole("tab" ...)` / heading / tab-click assertions) with link-based assertions:
```tsx
    expect(await screen.findByRole("link", { name: /files/i })).toBeInTheDocument();
    expect(screen.getByText("demo-project")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Files Page" })).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Reviews" }));
    expect(await screen.findByRole("heading", { name: "Reviews Page" })).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Audit" }));
    expect(await screen.findByRole("heading", { name: "Audit Page" })).toBeInTheDocument();
```
(The test already mocks `useSession` and `useProjectDetailQuery`; `useSpacesQuery` is unmocked and resolves to a loading/no-orgs state, which renders the user chip without crashing.)

- [ ] **Step 7: Verify lint + full suite**

Run: `npm run lint && npm test`
Expected: no tsc errors; all tests pass. The project sidebar now owns navigation; `demo-project` renders as sidebar text, not a heading.

- [ ] **Step 8: Commit**

```bash
git add apps/admin/src/components/layout/app-shell.tsx apps/admin/src/components/layout/project-workspace-layout.tsx apps/admin/src/components/layout/app-shell.test.tsx apps/admin/src/app/router.test.tsx apps/admin/src/lib/route-meta.ts
git commit -m "feat(admin): replace top-tab chrome with unified sidebar shell"
```

---

## Task 8: Shared `PageHeader`

**Files:**
- Create: `apps/admin/src/components/shared/page-header.tsx`
- Test: `apps/admin/src/components/shared/page-header.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `apps/admin/src/components/shared/page-header.test.tsx`:
```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { PageHeader } from "./page-header";

describe("PageHeader", () => {
  it("renders the title as a level-1 heading", () => {
    render(<PageHeader title="Tasks" />);
    expect(screen.getByRole("heading", { level: 1, name: "Tasks" })).toBeInTheDocument();
  });

  it("renders the optional description and actions", () => {
    render(
      <PageHeader
        actions={<button type="button">New task</button>}
        description="Background jobs"
        title="Tasks"
      />,
    );
    expect(screen.getByText("Background jobs")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New task" })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm test -- page-header`
Expected: FAIL — module `./page-header` not found.

- [ ] **Step 3: Implement `PageHeader`**

Create `apps/admin/src/components/shared/page-header.tsx`:
```tsx
import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

export function PageHeader({
  actions,
  breadcrumb,
  className,
  description,
  title,
}: {
  actions?: ReactNode;
  breadcrumb?: ReactNode;
  className?: string;
  description?: string;
  title: string;
}) {
  return (
    <header className={cn("flex flex-col gap-3 border-b border-border pb-4", className)}>
      {breadcrumb ? <div className="text-xs text-muted-foreground">{breadcrumb}</div> : null}
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid gap-1">
          <h1 className="text-lg font-semibold tracking-[-0.01em] text-foreground">{title}</h1>
          {description ? <p className="text-[13px] text-muted-foreground">{description}</p> : null}
        </div>
        {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
      </div>
    </header>
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm test -- page-header`
Expected: PASS.

- [ ] **Step 5: Verify lint + full suite**

Run: `npm run lint && npm test`
Expected: no tsc errors; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/components/shared/page-header.tsx apps/admin/src/components/shared/page-header.test.tsx
git commit -m "feat(admin): add shared PageHeader component"
```

---

## Task 9: Shared `StatusPill`

**Files:**
- Create: `apps/admin/src/components/shared/status-pill.tsx`
- Test: `apps/admin/src/components/shared/status-pill.test.tsx`

`StatusPill` renders the raw status string as a monospace label and colors it by mapping the value onto one of the five canonical status variants from the spec (`queued`, `running`, `succeeded`, `failed`, `retry_waiting`). Aliases collapse the app's other statuses: `completed`→`succeeded`, `cancelled`/`canceled`→`failed`, `pending`→`queued`, `in_progress`→`running`. Unknown values fall back to `queued`.

- [ ] **Step 1: Write the failing test**

Create `apps/admin/src/components/shared/status-pill.test.tsx`:
```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { StatusPill } from "./status-pill";

describe("StatusPill", () => {
  it("renders the raw status label", () => {
    render(<StatusPill value="running" />);
    expect(screen.getByText("running")).toBeInTheDocument();
  });

  it("maps a known status to its variant classes", () => {
    render(<StatusPill value="succeeded" />);
    expect(screen.getByText("succeeded").className).toContain("bg-[#ddf4e4]");
  });

  it("maps completed onto the succeeded variant", () => {
    render(<StatusPill value="completed" />);
    expect(screen.getByText("completed").className).toContain("bg-[#ddf4e4]");
  });

  it("falls back to the queued variant for unknown statuses", () => {
    render(<StatusPill value="mystery" />);
    expect(screen.getByText("mystery").className).toContain("bg-[#eaeef2]");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm test -- status-pill`
Expected: FAIL — module `./status-pill` not found.

- [ ] **Step 3: Implement `StatusPill`**

Create `apps/admin/src/components/shared/status-pill.tsx`:
```tsx
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const statusPillVariants = cva(
  "inline-flex items-center rounded-md border px-2 py-0.5 font-mono text-[10.5px] font-semibold leading-none",
  {
    variants: {
      status: {
        queued: "border-[#d0d7de] bg-[#eaeef2] text-[#57606a]",
        running: "border-[#b6d4fe] bg-[#ddebff] text-[#0a53c4]",
        succeeded: "border-[#abe0ba] bg-[#ddf4e4] text-[#1a7f37]",
        failed: "border-[#f5b5ba] bg-[#ffe3e3] text-[#c21f2e]",
        retry_waiting: "border-[#f3d98b] bg-[#fff1d6] text-[#9a6700]",
      },
    },
    defaultVariants: { status: "queued" },
  },
);

type StatusVariant = NonNullable<VariantProps<typeof statusPillVariants>["status"]>;

const STATUS_ALIASES: Record<string, StatusVariant> = {
  queued: "queued",
  pending: "queued",
  running: "running",
  in_progress: "running",
  succeeded: "succeeded",
  completed: "succeeded",
  failed: "failed",
  cancelled: "failed",
  canceled: "failed",
  retry_waiting: "retry_waiting",
};

export function StatusPill({ className, value }: { className?: string; value: string }) {
  const variant = STATUS_ALIASES[value] ?? "queued";
  return <span className={cn(statusPillVariants({ status: variant }), className)}>{value}</span>;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm test -- status-pill`
Expected: PASS (all four cases).

- [ ] **Step 5: Verify lint + full suite**

Run: `npm run lint && npm test`
Expected: no tsc errors; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/components/shared/status-pill.tsx apps/admin/src/components/shared/status-pill.test.tsx
git commit -m "feat(admin): add shared StatusPill with 5 status variants"
```

---

## Task 10: Shared `DataTable`

**Files:**
- Create: `apps/admin/src/components/shared/data-table.tsx`
- Test: `apps/admin/src/components/shared/data-table.test.tsx`

Generic table built on `@tanstack/react-table` + the official shadcn `Table`. Features: sortable column headers (when the column has an accessor), zebra rows, header styling, skeleton loading rows, a built-in empty row, and a pagination footer (`Prev`/`Next` + an `X of Y` count). Row actions are supplied by the consumer via an `actions`-style column cell.

- [ ] **Step 1: Write the failing test**

Create `apps/admin/src/components/shared/data-table.test.tsx`:
```tsx
import type { ColumnDef } from "@tanstack/react-table";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DataTable } from "./data-table";

type Row = { id: string; name: string };

const columns: ColumnDef<Row>[] = [{ accessorKey: "name", header: "Name" }];

describe("DataTable", () => {
  it("renders rows from data", () => {
    render(
      <DataTable
        columns={columns}
        data={[
          { id: "1", name: "alpha" },
          { id: "2", name: "beta" },
        ]}
      />,
    );
    expect(screen.getByText("alpha")).toBeInTheDocument();
    expect(screen.getByText("beta")).toBeInTheDocument();
  });

  it("shows the empty message when there is no data", () => {
    render(<DataTable columns={columns} data={[]} emptyMessage="No tasks." />);
    expect(screen.getByText("No tasks.")).toBeInTheDocument();
  });

  it("renders skeleton placeholders while loading", () => {
    const { container } = render(<DataTable columns={columns} data={[]} isLoading />);
    expect(container.querySelectorAll('[data-slot="skeleton"]').length).toBeGreaterThan(0);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm test -- data-table`
Expected: FAIL — module `./data-table` not found.

- [ ] **Step 3: Implement `DataTable`**

Create `apps/admin/src/components/shared/data-table.tsx`:
```tsx
import {
  type ColumnDef,
  type SortingState,
  flexRender,
  getCoreRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { ArrowDown, ArrowUp, ChevronsUpDown } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { cn } from "@/lib/utils";

type DataTableProps<TData, TValue> = {
  columns: ColumnDef<TData, TValue>[];
  data: TData[];
  emptyMessage?: string;
  isLoading?: boolean;
  pageSize?: number;
};

export function DataTable<TData, TValue>({
  columns,
  data,
  emptyMessage = "No results.",
  isLoading = false,
  pageSize = 10,
}: DataTableProps<TData, TValue>) {
  const [sorting, setSorting] = useState<SortingState>([]);

  const table = useReactTable({
    columns,
    data,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    initialState: { pagination: { pageSize } },
  });

  const rows = table.getRowModel().rows;

  return (
    <div className="grid gap-3">
      <div className="overflow-hidden rounded-md border border-border">
        <Table>
          <TableHeader>
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow className="bg-muted hover:bg-muted" key={headerGroup.id}>
                {headerGroup.headers.map((header) => {
                  const canSort = header.column.getCanSort();
                  const sorted = header.column.getIsSorted();
                  return (
                    <TableHead
                      className="text-[11px] uppercase tracking-[0.04em] text-muted-foreground"
                      key={header.id}
                    >
                      {header.isPlaceholder ? null : canSort ? (
                        <button
                          className="inline-flex items-center gap-1 hover:text-foreground"
                          onClick={header.column.getToggleSortingHandler()}
                          type="button"
                        >
                          {flexRender(header.column.columnDef.header, header.getContext())}
                          {sorted === "asc" ? (
                            <ArrowUp className="size-3" />
                          ) : sorted === "desc" ? (
                            <ArrowDown className="size-3" />
                          ) : (
                            <ChevronsUpDown className="size-3 opacity-50" />
                          )}
                        </button>
                      ) : (
                        flexRender(header.column.columnDef.header, header.getContext())
                      )}
                    </TableHead>
                  );
                })}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {isLoading ? (
              Array.from({ length: 5 }).map((_, rowIndex) => (
                <TableRow key={`skeleton-${rowIndex}`}>
                  {columns.map((_column, colIndex) => (
                    <TableCell key={`skeleton-${rowIndex}-${colIndex}`}>
                      <Skeleton className="h-4 w-full" />
                    </TableCell>
                  ))}
                </TableRow>
              ))
            ) : rows.length ? (
              rows.map((row, index) => (
                <TableRow className={cn(index % 2 === 1 && "bg-[#fbfcfd]")} key={row.id}>
                  {row.getVisibleCells().map((cell) => (
                    <TableCell key={cell.id}>
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell
                  className="h-24 text-center text-sm text-muted-foreground"
                  colSpan={columns.length}
                >
                  {emptyMessage}
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span>
          {rows.length} of {data.length}
        </span>
        <div className="flex gap-2">
          <Button
            disabled={!table.getCanPreviousPage()}
            onClick={() => table.previousPage()}
            size="sm"
            variant="outline"
          >
            Prev
          </Button>
          <Button
            disabled={!table.getCanNextPage()}
            onClick={() => table.nextPage()}
            size="sm"
            variant="outline"
          >
            Next
          </Button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm test -- data-table`
Expected: PASS (rows render, empty message shows, skeletons present while loading).

- [ ] **Step 5: Verify lint + full suite**

Run: `npm run lint && npm test`
Expected: no tsc errors; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/components/shared/data-table.tsx apps/admin/src/components/shared/data-table.test.tsx
git commit -m "feat(admin): add shared DataTable on @tanstack/react-table"
```

---

## Task 11: Shared `FilterToolbar`

**Files:**
- Create: `apps/admin/src/components/shared/filter-toolbar.tsx`
- Test: `apps/admin/src/components/shared/filter-toolbar.test.tsx`

A controlled search `Input` (with a leading magnifier icon) plus a `children` slot for future filter controls. Foundation uses search only; filter `Select`s are added by per-group plans once `Select` is migrated to Radix.

- [ ] **Step 1: Write the failing test**

Create `apps/admin/src/components/shared/filter-toolbar.test.tsx`:
```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { FilterToolbar } from "./filter-toolbar";

describe("FilterToolbar", () => {
  it("renders the search input with placeholder", () => {
    render(
      <FilterToolbar onSearchChange={() => {}} searchPlaceholder="Filter tasks..." searchValue="" />,
    );
    expect(screen.getByPlaceholderText("Filter tasks...")).toBeInTheDocument();
  });

  it("calls onSearchChange when typing", async () => {
    const user = userEvent.setup();
    const onSearchChange = vi.fn();
    render(<FilterToolbar onSearchChange={onSearchChange} searchValue="" />);
    await user.type(screen.getByRole("textbox"), "a");
    expect(onSearchChange).toHaveBeenCalledWith("a");
  });

  it("renders children alongside the search box", () => {
    render(
      <FilterToolbar onSearchChange={() => {}} searchValue="">
        <button type="button">Status</button>
      </FilterToolbar>,
    );
    expect(screen.getByRole("button", { name: "Status" })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm test -- filter-toolbar`
Expected: FAIL — module `./filter-toolbar` not found.

- [ ] **Step 3: Implement `FilterToolbar`**

Create `apps/admin/src/components/shared/filter-toolbar.tsx`:
```tsx
import { Search } from "lucide-react";
import type { ReactNode } from "react";

import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

export function FilterToolbar({
  children,
  className,
  onSearchChange,
  searchPlaceholder = "Filter...",
  searchValue,
}: {
  children?: ReactNode;
  className?: string;
  onSearchChange: (value: string) => void;
  searchPlaceholder?: string;
  searchValue: string;
}) {
  return (
    <div className={cn("flex flex-wrap items-center gap-2", className)}>
      <div className="relative max-w-xs flex-1">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          className="pl-8"
          onChange={(event) => onSearchChange(event.target.value)}
          placeholder={searchPlaceholder}
          value={searchValue}
        />
      </div>
      {children}
    </div>
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm test -- filter-toolbar`
Expected: PASS.

- [ ] **Step 5: Verify lint + full suite**

Run: `npm run lint && npm test`
Expected: no tsc errors; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/components/shared/filter-toolbar.tsx apps/admin/src/components/shared/filter-toolbar.test.tsx
git commit -m "feat(admin): add shared FilterToolbar"
```

---

## Task 12: Shared `states` (Loading / Error / Forbidden)

**Files:**
- Create: `apps/admin/src/components/shared/states.tsx`
- Test: `apps/admin/src/components/shared/states.test.tsx`

Lightweight, reusable inline state panes for embedding inside cards/content regions. (`RouteStatePane` and `EmptyState` remain for full-route panes; these are the compact, composable variants the spec calls for.)

- [ ] **Step 1: Write the failing test**

Create `apps/admin/src/components/shared/states.test.tsx`:
```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ErrorState, ForbiddenState, LoadingState } from "./states";

describe("shared states", () => {
  it("LoadingState renders the requested number of skeletons", () => {
    const { container } = render(<LoadingState rows={3} />);
    expect(container.querySelectorAll('[data-slot="skeleton"]').length).toBe(3);
  });

  it("ErrorState renders title and description", () => {
    render(<ErrorState description="It failed" title="Boom" />);
    expect(screen.getByText("Boom")).toBeInTheDocument();
    expect(screen.getByText("It failed")).toBeInTheDocument();
  });

  it("ForbiddenState renders a default access message", () => {
    render(<ForbiddenState />);
    expect(screen.getByText("Access denied")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm test -- shared/states`
Expected: FAIL — module `./states` not found.

- [ ] **Step 3: Implement `states.tsx`**

Create `apps/admin/src/components/shared/states.tsx`:
```tsx
import { AlertTriangle, ShieldAlert } from "lucide-react";
import type { ReactNode } from "react";

import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

export function LoadingState({ className, rows = 4 }: { className?: string; rows?: number }) {
  return (
    <div aria-label="Loading" className={cn("grid gap-2", className)} role="status">
      {Array.from({ length: rows }).map((_, index) => (
        <Skeleton className="h-10 w-full" key={index} />
      ))}
    </div>
  );
}

function MessagePane({
  action,
  description,
  icon,
  title,
}: {
  action?: ReactNode;
  description?: string;
  icon: ReactNode;
  title: string;
}) {
  return (
    <div className="grid place-items-center gap-2 rounded-md border border-border bg-card p-8 text-center">
      <div className="text-muted-foreground">{icon}</div>
      <p className="text-sm font-semibold text-foreground">{title}</p>
      {description ? <p className="text-[13px] text-muted-foreground">{description}</p> : null}
      {action}
    </div>
  );
}

export function ErrorState({
  action,
  description,
  title = "Something went wrong",
}: {
  action?: ReactNode;
  description?: string;
  title?: string;
}) {
  return (
    <MessagePane
      action={action}
      description={description}
      icon={<AlertTriangle className="size-5" />}
      title={title}
    />
  );
}

export function ForbiddenState({
  description = "You do not have permission to view this resource.",
  title = "Access denied",
}: {
  description?: string;
  title?: string;
}) {
  return (
    <MessagePane description={description} icon={<ShieldAlert className="size-5" />} title={title} />
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm test -- shared/states`
Expected: PASS.

- [ ] **Step 5: Verify lint + full suite**

Run: `npm run lint && npm test`
Expected: no tsc errors; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/components/shared/states.tsx apps/admin/src/components/shared/states.test.tsx
git commit -m "feat(admin): add shared inline loading/error/forbidden states"
```

---

## Task 13: Migrate the Tasks screen onto the shared layer

**Files:**
- Modify: `apps/admin/src/features/tasks/page.tsx`
- Modify: `apps/admin/src/features/tasks/page.test.tsx`

This is the reference migration: replace `PageSection` with `PageHeader`, the hand-built left `Table` with `DataTable`, and `StatusBadge` with `StatusPill`. The master/detail layout, the auto-select `useEffect`, and the right-hand detail card are preserved (their behavior is asserted by the existing test).

- [ ] **Step 1: Update the test first (assertions reflect the new chrome)**

In `apps/admin/src/features/tasks/page.test.tsx`, change only the first assertion of the test body — the project name is now rendered by the sidebar as text, not by a removed `ProjectHeader` heading. Replace:
```tsx
    expect(await screen.findByRole("heading", { name: "demo-project" })).toBeInTheDocument();
```
with:
```tsx
    expect(await screen.findByText("demo-project")).toBeInTheDocument();
```
Leave the other three assertions unchanged (`getByRole("heading", { name: "Tasks" })` now comes from `PageHeader`; the `"Task no longer exists"` heading and its description still come from `RouteStatePane` in the detail pane).

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm test -- tasks/page`
Expected: FAIL — the page still imports `PageSection`/`StatusBadge` and renders the old table; the `findByText("demo-project")` may pass but the new structure is not in place yet. (If it passes by accident, Step 4 still re-verifies the full new render.)

- [ ] **Step 3: Rewrite `page.tsx` onto the shared components**

Replace the entire contents of `apps/admin/src/features/tasks/page.tsx` with:
```tsx
import type { ColumnDef } from "@tanstack/react-table";
import { useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { RouteStatePane } from "@/components/layout/route-state-pane";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { normalizeAppError } from "@/lib/app-error";

import { ProjectFileLink } from "../shared/file-links";
import {
  useCancelTaskMutation,
  useProjectTasksQuery,
  useRetryTaskMutation,
  useTaskDetailQuery,
} from "./queries";

type ProjectTask = NonNullable<ReturnType<typeof useProjectTasksQuery>["data"]>[number];

export function TasksPage() {
  const { projectId = "" } = useParams();
  const tasks = useProjectTasksQuery(projectId);
  const retryTask = useRetryTaskMutation();
  const cancelTask = useCancelTaskMutation();
  const [selectedTaskId, setSelectedTaskId] = useState("");

  const taskList = tasks.data ?? [];

  useEffect(() => {
    if (!selectedTaskId && tasks.data?.[0]?.id) {
      setSelectedTaskId(tasks.data[0].id);
    }
  }, [selectedTaskId, tasks.data]);

  const detail = useTaskDetailQuery(projectId, selectedTaskId);
  const detailError = detail.error ? normalizeAppError(detail.error) : null;

  const columns = useMemo<ColumnDef<ProjectTask>[]>(
    () => [
      {
        accessorKey: "status",
        header: "Status",
        cell: ({ row }) => <StatusPill value={row.original.status} />,
      },
      {
        accessorKey: "title",
        header: "Task",
        cell: ({ row }) => (
          <div className="grid gap-1">
            <button
              className="text-left font-medium text-foreground underline-offset-4 hover:underline"
              onClick={() => setSelectedTaskId(row.original.id)}
              type="button"
            >
              {row.original.title}
            </button>
            <span className="font-mono text-[10.5px] text-muted-foreground">{row.original.id}</span>
          </div>
        ),
      },
      {
        accessorKey: "taskType",
        header: "Type",
        cell: ({ row }) => (
          <span className="font-mono text-[11.5px] text-muted-foreground">
            {row.original.taskType}
          </span>
        ),
      },
      {
        id: "path",
        header: "Path",
        cell: ({ row }) =>
          row.original.relativePath ? (
            <ProjectFileLink path={row.original.relativePath} projectId={projectId} />
          ) : (
            <span className="text-sm text-muted-foreground">-</span>
          ),
      },
      {
        id: "attempts",
        header: "Attempts",
        cell: ({ row }) => (
          <span className="text-sm text-muted-foreground">
            {(row.original.attemptCount ?? 0).toString()}/
            {(row.original.maxAttempts ?? 0).toString()}
          </span>
        ),
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) => (
          <div className="flex flex-wrap justify-end gap-2">
            <Button onClick={() => setSelectedTaskId(row.original.id)} size="sm" variant="outline">
              Inspect
            </Button>
            <Button
              onClick={() => retryTask.mutateAsync({ projectId, taskId: row.original.id })}
              size="sm"
              variant="secondary"
            >
              Retry
            </Button>
            {!["completed", "succeeded", "failed", "cancelled"].includes(row.original.status) ? (
              <Button
                onClick={() => cancelTask.mutateAsync({ projectId, taskId: row.original.id })}
                size="sm"
                variant="outline"
              >
                Cancel
              </Button>
            ) : null}
          </div>
        ),
      },
    ],
    [cancelTask, projectId, retryTask],
  );

  return (
    <div className="grid gap-4">
      <PageHeader
        description="Inspect queued work, retry failed jobs, and examine task payloads."
        title="Tasks"
      />
      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.4fr)_380px]">
        <div className="grid gap-3">
          {tasks.error ? (
            <RouteStatePane
              description={normalizeAppError(tasks.error).message}
              state="failed"
              title="Tasks unavailable"
            />
          ) : (
            <DataTable
              columns={columns}
              data={taskList}
              emptyMessage="No tasks have been queued for this project yet."
              isLoading={tasks.isLoading}
            />
          )}
        </div>

        <Card>
          <CardHeader>
            <CardTitle>Task Detail</CardTitle>
            <CardDescription>
              Selected task payload, status, and execution metadata.
            </CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4">
            {!selectedTaskId ? (
              <EmptyState
                description="Select a task from the table to inspect its payload and result."
                title="No task selected"
              />
            ) : detailError ? (
              <RouteStatePane
                description={
                  detailError.kind === "not_found"
                    ? "The selected task disappeared or no longer exists."
                    : detailError.message
                }
                state={detailError.kind === "not_found" ? "not_found" : "failed"}
                title={detailError.kind === "not_found" ? "Task no longer exists" : "Task unavailable"}
              />
            ) : detail.data ? (
              <div className="grid gap-4">
                <div className="flex flex-wrap items-center gap-3">
                  <StatusPill value={detail.data.status} />
                  <span className="text-sm font-medium">{detail.data.title}</span>
                </div>
                <div className="grid gap-2 text-sm text-muted-foreground">
                  <p className="font-mono text-xs">{detail.data.taskType}</p>
                  {detail.data.relativePath ? (
                    <ProjectFileLink path={detail.data.relativePath} projectId={projectId} />
                  ) : null}
                </div>
                {detail.data.error ? (
                  <Card>
                    <CardHeader>
                      <CardTitle>Error</CardTitle>
                    </CardHeader>
                    <CardContent className="pt-0">
                      <pre className="whitespace-pre-wrap break-words text-xs">
                        {JSON.stringify(detail.data.error, null, 2)}
                      </pre>
                    </CardContent>
                  </Card>
                ) : null}
                {detail.data.result ? (
                  <Card>
                    <CardHeader>
                      <CardTitle>Result</CardTitle>
                    </CardHeader>
                    <CardContent className="pt-0">
                      <pre className="whitespace-pre-wrap break-words text-xs">
                        {JSON.stringify(detail.data.result, null, 2)}
                      </pre>
                    </CardContent>
                  </Card>
                ) : null}
                <Card>
                  <CardHeader>
                    <CardTitle>Detail</CardTitle>
                  </CardHeader>
                  <CardContent className="pt-0">
                    <pre className="whitespace-pre-wrap break-words text-xs">
                      {JSON.stringify(detail.data.detail, null, 2)}
                    </pre>
                  </CardContent>
                </Card>
              </div>
            ) : (
              <EmptyState description="Task data is not available yet." title="Loading task detail" />
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm test -- tasks/page`
Expected: PASS — `demo-project` (sidebar text), `Tasks` (PageHeader h1), `Task no longer exists` (detail RouteStatePane), and the disappeared-task description all present.

- [ ] **Step 5: Verify the screen in the browser**

Run: `npm run dev`, open a project's Tasks screen. Confirm: PageHeader title + description, the DataTable with monospace task IDs/types, colored `StatusPill`s, sortable Status/Task/Type headers, Prev/Next pagination, and the detail pane updating when a row's task title or `Inspect` is clicked. Stop the server.

- [ ] **Step 6: Verify lint + full suite**

Run: `npm run lint && npm test`
Expected: no tsc errors; all tests pass.

- [ ] **Step 7: Commit**

```bash
git add apps/admin/src/features/tasks/page.tsx apps/admin/src/features/tasks/page.test.tsx
git commit -m "feat(admin): migrate Tasks screen to PageHeader + DataTable + StatusPill"
```

---

## Task 14: Remove dead `styles.css` + final verification

**Files:**
- Delete: `apps/admin/src/styles.css`

- [ ] **Step 1: Confirm `styles.css` has no importers**

Run: `grep -rn "styles.css" apps/admin/src`
Expected: no results (only `index.css` is imported, by `src/main.tsx`). If any importer exists, stop and reassess before deleting.

- [ ] **Step 2: Delete the dead stylesheet**

```bash
git rm apps/admin/src/styles.css
```

- [ ] **Step 3: Final full verification (lint + tests + production build)**

Run: `npm run lint && npm test && npm run build`
Expected: tsc clean; all tests pass; Vite production build succeeds with no unresolved imports.

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/styles.css
git commit -m "chore(admin): delete dead styles.css"
```

---

## Foundation done — what comes next

After this plan executes, the shared layer exists and Tasks proves it. Follow-up per-group plans migrate the remaining screens onto the same components and perform the deferred Radix migrations co-located with their consumers:

- **Global screens plan:** Login, Dashboard, Projects, Users, API Tokens, Settings, Orgs/Teams.
- **Content + Explore plan:** Files, Sources, Source Watch, Search, Chat (markdown), Graph, Deep Research — includes migrating `Select` (files), `Tabs` (sources), and `Dialog` consumers to Radix.
- **Quality + System + Overview plan:** Lint, Reviews, Dedup, Audit, and the project Overview hero (folding in `project-header.tsx`) — includes migrating `Select` (dedup, reviews) to Radix.

---
