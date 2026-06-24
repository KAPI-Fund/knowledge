# Admin UI Redesign — Design Spec

**Date:** 2026-06-24
**Topic:** Complete visual redesign and IA restructure of `apps/admin`
**Status:** Approved direction; ready for implementation planning.

## Goal

Replace the current "half-finished" admin UI with a polished, consistent, data-forward
design system covering every existing screen. No new product features and no backend
changes — this is a visual + information-architecture redesign only.

## Decisions (locked during brainstorming)

| Decision | Choice |
|---|---|
| Scope | Full restructure — IA (navigation) **and** visual system |
| Theme | **Light only** (no dark mode) |
| Aesthetic | **Technical Console** — dense, data-forward, GitHub/Datadog feel |
| Accent | **Blue `#1F6FEB`** |
| Neutral | Cool **slate** (GitHub-like grays) |
| Corners | Sharp, ~6px radius |
| Type | Sans for UI, **monospace for IDs / paths / code / task types** |
| Navigation | **Unified left sidebar** that swaps between global and project context |
| Components | **shadcn/ui only** — compose primitives, never hand-roll one-off components |
| Rollout | **Foundation-first** — build the shared layer, then migrate screens |

> Implementation rule (from user): build UI by composing shadcn/ui primitives from
> `apps/admin/src/components/ui/`. When a primitive is missing (e.g. Sidebar), add it
> from shadcn rather than inventing one.

## Architecture overview

The app keeps its current stack: React 19, React Router 7, TanStack Query 5, Tailwind
CSS v4 (CSS `@theme`), shadcn/ui (`cn` util), lucide-react, vitest + Testing Library.

Three layers change:

1. **Design tokens** (`src/index.css`) — recolored to the Blue/slate Technical Console
   palette, sharper radius, mono font added, gradient background removed.
2. **App shell & navigation** — a route-aware unified sidebar replaces the current
   `SystemSidebar` + `TopBar` + project top-tabs (`ProjectNav`/`.subnav`).
3. **Shared presentation components** — `DataTable`, `PageHeader`, `FilterToolbar`,
   `StatusPill`, form-section primitives, and unified loading/empty/error states, all
   reused across screens.

Feature pages are then migrated onto this layer group by group.

---

## Section A — Information architecture & navigation

### Unified contextual sidebar

A single left sidebar (built on shadcn **Sidebar**: `SidebarProvider`, `Sidebar`,
`SidebarHeader`, `SidebarContent`, `SidebarGroup`, `SidebarGroupLabel`, `SidebarMenu`,
`SidebarMenuItem`, `SidebarMenuButton`, `SidebarFooter`, `SidebarTrigger`). It renders
one of two contexts based on the current route:

**Global context** (routes outside `/projects/:projectId`):
- Single menu group: Dashboard, Projects, Users, API Tokens, Settings.
- Footer: user chip (avatar + username) with sign-out / space switcher.

**Project context** (routes matching `/projects/:projectId/*`):
- Header: `← All projects` link, project name, `prj_id` rendered in monospace.
- Grouped menu:
  - **Overview** (ungrouped top item)
  - **Content** — Files, Sources, Source Watch
  - **Explore** — Search, Chat, Graph, Deep Research
  - **Quality** — Lint, Reviews, Dedup
  - **System** — Tasks, Audit
- Footer: user chip.

Active item uses the Blue accent (filled background). Org/Team routes (`/orgs/...`)
reuse the global shell.

### Routing & layout impact
- `ProjectWorkspaceLayout` no longer renders the top-tab `ProjectNav`; the sidebar owns
  navigation. It renders the `Outlet` only.
- Project name + stats move to the **Overview** page hero (sidebar shows the name; the
  page header shows the current screen title).
- `src/lib/route-meta.ts` `projectRoutes` is restructured from a flat list into the
  grouped structure above (sections with items). `systemRoutes` stays a flat list.

### Files touched
- Rework: `components/layout/app-shell.tsx`, `project-workspace-layout.tsx`,
  `project-header.tsx`.
- New: `components/layout/app-sidebar.tsx` (route-aware global/project nav).
- Remove (after migration): `components/layout/system-sidebar.tsx`,
  `components/layout/top-bar.tsx`, and `.subnav` usage.
- New shadcn primitives: `components/ui/sidebar.tsx`, `dropdown-menu.tsx`, `avatar.tsx`.

---

## Section B — Visual system (design tokens)

Rewrite `:root` in `src/index.css`. Values below are the source of truth (hex);
express as hex or oklch in the CSS variables (Tailwind v4 accepts either).

### Color tokens
| Token | Value | Use |
|---|---|---|
| `--background` | `#FBFCFD` | App/content background (flat — remove gradient) |
| `--card` | `#FFFFFF` | Cards / surfaces |
| `--muted` (sidebar surface) | `#F6F7F8` | Sidebar, table header, toolbars |
| `--foreground` | `#0D1117` | Primary text |
| `--muted-foreground` | `#57606A` | Secondary text |
| subtle foreground | `#8B949E` | IDs, meta, placeholders |
| `--border` | `#D0D7DE` | Borders, dividers |
| row divider | `#EAEEF2` | Table row separators, zebra `#FBFCFD` |
| `--primary` | `#1F6FEB` | Accent: buttons, active nav, links, focus |
| `--primary-foreground` | `#FFFFFF` | On-accent text |
| primary hover | `#1A63D6` | Button/link hover |
| `--ring` | `#1F6FEB` | Focus ring (border + 3px @ ~18% alpha glow) |
| `--destructive` | `#C21F2E` | Destructive actions |

### Status colors (shared; used by `StatusPill`)
| Status | bg | text | border |
|---|---|---|---|
| `queued` | `#EAEEF2` | `#57606A` | `#D0D7DE` |
| `running` | `#DDEBFF` | `#0A53C4` | `#B6D4FE` |
| `succeeded` | `#DDF4E4` | `#1A7F37` | `#ABE0BA` |
| `failed` | `#FFE3E3` | `#C21F2E` | `#F5B5BA` |
| `retry_waiting` | `#FFF1D6` | `#9A6700` | `#F3D98B` |

### Typography
- **Sans:** keep Geist Variable for UI.
- **Mono:** add `@fontsource-variable/geist-mono` (or `ui-monospace` stack) exposed as
  `--font-mono`; used for IDs, file paths, task types, code, token prefixes.
- Scale: page title 18px/600 (tracking -0.01em), section heading 14px/600, body 13px,
  small 12px, micro/label 11px uppercase (tracking +0.04em).

### Radius & spacing
- `--radius: 0.375rem` (6px). Recompute `--radius-sm/md/lg/xl` proportionally.
- Tighter spacing: page padding 14–18px, table row padding 10px 14px, card padding 16px,
  base unit 4px.

### Background
- Remove the radial + linear gradient body background; use flat `--background`.

### Cleanup
- Delete dead `src/styles.css` (already unused — `main.tsx` imports only `index.css`).
- Remove legacy `@layer components` semantic classes (`.shell`, `.sidebar`, `.subnav`,
  `.files-layout`, `.graph-layout`, `.tree-list`, `.preview-pane`, `.ghost-button`,
  `.stats`, `.panel`, etc.) **as their consumers migrate** to components — not before.

---

## Section C — Shared foundation components (built first)

Location: new `components/shared/` for cross-feature pieces.

1. **AppShell + AppSidebar** — shadcn Sidebar; route-aware context (Section A).
2. **PageHeader** — `{ title, description?, actions?, breadcrumb? }`. Standard header on
   every content page.
3. **DataTable** — generic table built on shadcn **Table** + `@tanstack/react-table`
   (add dep) + shadcn **DropdownMenu** for row actions. Features: column config,
   sortable headers, zebra rows, row hover, kebab row-action menu, pagination footer,
   skeleton loading rows, built-in empty state. Used by all list screens.
4. **StatusPill** — extends shadcn **Badge** with the status variants above; monospace
   label. (Replaces `components/layout/status-badge.tsx`.)
5. **FilterToolbar** — search `Input` + filter `Select`s in a consistent toolbar row.
6. **Form primitives** — `FormSection` (titled card with description + responsive field
   grid) and `Field` (label + control + hint/error). Layout wrappers around existing
   shadcn `Input`/`Select`/`Textarea`; no new form library.
7. **States** — `LoadingState` (skeletons), refined `EmptyState` (less verbose),
   `ErrorState` / `ForbiddenState` (refine `route-state-pane.tsx`).
8. **Retune existing primitives** to the new tokens: `button` (blue primary, 6px),
   `card` (hairline border, minimal shadow), `input`/`select`/`textarea` (blue focus
   ring), `badge`, `tabs` (for in-page sub-tabs), `separator`, `tooltip`, `dialog`,
   `sheet`, `scroll-area`, `skeleton`, `alert`.

### New dependencies
- `@tanstack/react-table` (DataTable)
- `react-markdown` (Chat assistant rendering)
- `@fontsource-variable/geist-mono` (mono font)
- shadcn additions pulling Radix: `dropdown-menu`, `avatar` (and `sidebar` infra).

---

## Section D — Per-screen treatment

### Global screens
- **Login** (`features/auth/login-page`) — centered auth card, product mark, blue
  primary submit, inline error.
- **Dashboard** (`features/dashboard/page`) — polished stat/overview cards grid +
  recent activity.
- **Projects** (`features/projects/page`) — DataTable (name, `rootPath` mono, counts,
  createdAt, row actions) + "New project" + search.
- **Users** (`features/users/page`) — DataTable (username, role badge, created, actions).
- **API Tokens** (`features/api-tokens/page`) — DataTable (name, token prefix mono,
  scopes, created, last used, revoke) + create dialog showing the token once.
- **Settings** (`features/settings/page`) — the 20+ linear inputs reorganized into
  grouped `FormSection` cards (e.g. General, LLM/Search providers, API keys, Web search).
- **Orgs/Teams** (`features/orgs/*`, `features/teams/*`) — raw HTML tables migrated to
  DataTable; pages reskinned to the shell.

### Project screens
- **Overview** (`features/projects/detail-page`) — hero (name + `prj_id` + stats) + quick
  stat cards + recent tasks/activity.
- **Files** (`features/files/*`) — tree (≈320px) + preview/editor pane. Tree rebuilt as a
  proper component (indentation via nesting, not inline styles), monospace names, blue
  selected state; `wiki-page-editor` reskinned.
- **Sources** (`features/sources/page`) — sources list/table + add-source flow.
- **Source Watch** (`features/source-watch/page`) — watch configs list + status pills.
- **Search** (`features/search/page`) — search bar + result cards (score, path mono,
  snippet).
- **Chat** (`features/chat/*`) — user/assistant message bubbles, **markdown rendering**
  for assistant, streaming indicator, bottom composer.
- **Lint** (`features/lint/page`) — run controls + issues list/table with severity pills.
- **Graph** (`features/graph/*`) — sigma canvas with a sharp-bordered container; the
  filters / insights / legend / node-detail panels reskinned as cards; controls as
  shadcn buttons. Keep the existing `graph-layout` two-column concept as a component.
- **Tasks** (`features/tasks/page`) — DataTable per the approved mockup; task detail
  (dialog or route) showing status, attempts, payload, result, error.
- **Reviews** (`features/reviews/page`) — DataTable / list of review items + actions.
- **Dedup** (`features/dedup/page`) — duplicate candidates with merge actions.
- **Deep Research** (`features/deep-research/page`) — research runs list + detail.
- **Audit** (`features/audit/page`) — DataTable of audit entries (actor, action, target,
  timestamp mono).

---

## Section E — States & polish

- Consistent loading skeletons (table rows, cards).
- Refined, concise empty states (icon + one line + primary action).
- Unified error / forbidden / not-found panes via `route-state-pane`.
- Blue focus rings everywhere; visible hover feedback on rows, buttons, nav items.
- Monospace consistently applied to IDs, paths, task types, token prefixes.

---

## Section F — Testing impact

- `components/layout/app-shell.test.tsx` and `app/router.test.tsx` currently assert
  **tab** roles (`getByRole("tab", ...)`) and the old nav. With navigation in the
  sidebar, these become links / menu items — update both tests to the new nav structure
  (assert sidebar nav items and active state instead of tabs).
- Keep per-feature test mocks working; update any that assert removed chrome
  (`TopBar`, `ProjectNav`).
- Each new shared component (`DataTable`, `PageHeader`, `StatusPill`, `FilterToolbar`,
  states) gets unit tests.

---

## Non-goals

- No dark mode.
- No backend / API changes.
- No new product features.
- No IA changes beyond grouping nav (no merging, adding, or removing screens).
- No data-fetching / query refactors.

---

## File-structure summary

```
apps/admin/src/
  index.css                      # token rewrite, mono font, flat bg, 6px radius
  styles.css                     # DELETE (dead)
  lib/route-meta.ts              # projectRoutes -> grouped sections
  components/ui/
    sidebar.tsx  dropdown-menu.tsx  avatar.tsx   # ADD (shadcn)
    button/card/input/select/badge/...           # RETUNE to tokens
  components/layout/
    app-shell.tsx                # rework (SidebarProvider shell)
    app-sidebar.tsx              # NEW (route-aware nav)
    project-workspace-layout.tsx # drop top-tabs
    project-header.tsx           # fold into Overview hero
    route-state-pane.tsx empty-state.tsx  # refine
    status-badge.tsx -> status-pill.tsx
    system-sidebar.tsx top-bar.tsx        # REMOVE after migration
  components/shared/ (new)
    data-table.tsx page-header.tsx filter-toolbar.tsx
    form-section.tsx states.tsx
  features/**/page.tsx           # migrate onto the shared layer
```
