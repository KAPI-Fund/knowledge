# Admin Shadcn Workbench Design

## Summary

This document defines a full UI redesign for `apps/admin`. The goal is to replace the current plain HTML/CSS frontend with a shadcn-style component system and a route-centered operations shell while preserving all existing admin functionality, routes, and API contracts.

The redesign is a presentation and interaction overhaul, not a backend rewrite. Existing query and mutation hooks remain the source of truth for behavior. The UI should feel like a real internal control plane rather than a scaffold.

## Goals

- Rebuild the admin app with a consistent shadcn-style visual system.
- Preserve every existing admin route and keep each one functional.
- Keep the app route-centered, with system-level navigation always visible.
- Make project operations, task monitoring, search, reviews, audit logs, and settings usable end to end.
- Keep the UI aligned with the operational density and clarity of `llm_wiki`, without copying its desktop-only runtime model.

## Non-Goals

- No backend business-logic rewrite.
- No API contract changes are required for this redesign.
- No new admin capabilities beyond the current backend surface.
- No chat-style product shell.
- No broad snapshot-test expansion.

## Current State

The current admin app already has:

- route structure for dashboard, projects, project detail, files, sources, source watch, search, query, lint, graph, tasks, reviews, audit, users, and settings
- working API hooks for project and system operations
- login/session handling

What is missing is the product layer:

- no shadcn component system
- no consistent layout shell
- no dense tables, cards, dialogs, or tabs
- no real visual hierarchy
- no cohesive treatment of loading, error, and empty states

## Architecture

### UI Stack

The admin app should adopt a shadcn-style component layer under `apps/admin/src/components/ui` and a shared utility layer for class composition and tokens.

The frontend bootstrap should add the styling and component-generation scaffold needed for that layer, including:

- Tailwind-based token and theme setup for `apps/admin`
- shadcn component generation config for the admin app
- shared path aliases for component and utility imports
- a replacement for the current plain `styles.css` layout layer

Minimum shared primitives:

- `Button`
- `Input`
- `Textarea`
- `Select`
- `Card`
- `Badge`
- `Tabs`
- `Dialog`
- `Sheet`
- `Table`
- `Tooltip`
- `Separator`
- `ScrollArea`
- `Alert`
- `Skeleton`

### Shell Layout

The app shell should be route-centered and persistent:

- left primary navigation for system routes
- top bar with breadcrumb/project context, current user info, and key actions
- main content area for the active route
- project-level secondary navigation under the project header
- responsive collapse to a sheet on smaller screens

This shell replaces the current bare sidebar/content split.

### Shell Components

The shell should be built from small, bounded components:

- `AppShell`
  - owns the overall authenticated layout and route framing
- `ProjectWorkspaceLayout`
  - composes `ProjectHeader`, `ProjectNav`, and the project route outlet
- `SystemSidebar`
  - renders the primary navigation for dashboard, projects, users, and settings
- `TopBar`
  - renders breadcrumb context, current user, and global actions
- `ProjectHeader`
  - renders the current project title, root path, and summary counts
- `ProjectNav`
  - renders the project-level route tabs or segmented navigation
- `PageSection`
  - standardizes section spacing, headings, and action slots
- `EmptyState`
  - renders intentful empty conditions with a next action
- `ActionBar`
  - groups primary and secondary actions for a page or panel

### Shell Boundaries

Use the following ownership boundaries so the shell units remain independently understandable and testable.

| Unit | Owns | Inputs | Network/Data Ownership | Render Conditions |
|------|------|--------|------------------------|------------------|
| `AppShell` | auth gate, outer layout, responsive sidebar/sheet, route outlet | current route, session state | owns `useSession` only | all authenticated routes; never renders on `/login` |
| `ProjectWorkspaceLayout` | project-scoped chrome, project route-state handoff, child outlet framing | `projectId` route param, current location | owns `useProjectDetailQuery(projectId)` and provides derived project context to descendants | only on `/projects/:projectId` and descendants |
| `SystemSidebar` | top-level navigation state | current location | none | renders inside `AppShell` |
| `TopBar` | breadcrumb display, user identity surface, global action slot | current location, authenticated user, optional project label from project chrome | no direct project query ownership | renders inside `AppShell` |
| `ProjectHeader` | project-scoped title, root path, summary strip | project detail context | none directly; reads from `ProjectWorkspaceLayout` context | only inside `ProjectWorkspaceLayout` when project state is `ready` |
| `ProjectNav` | project route tabs / segmented navigation | `projectId`, current location | none | only when the route is project-scoped and the project route is not in a forbidden/not-found state |
| `PageSection` | page spacing, title, subtitle, action row | static props | none | any page |
| `EmptyState` | zero-data and next-step presentation | title, description, action | none | any page when data is empty but valid |
| `ActionBar` | primary/secondary action grouping | children/actions | none | any page section |

Project-scoped chrome appears only on `/projects/:projectId` routes. `/projects`, `/users`, and `/settings` stay in the system shell without project header or project tabs.

Breadcrumb generation should follow route metadata, not inferred network state. When a breadcrumb needs the project name, `TopBar` receives it from `ProjectHeader` or a project-layout context rather than issuing its own duplicate query.

`ProjectWorkspaceLayout` should expose a narrow descendant interface:

- `project`
  - id, name, rootPath, createdAt, sourceCount, taskCount, reviewCount
- `projectState`
  - `loading | ready | forbidden | not_found | failed`
- `projectError`
  - normalized error or `null`

### Data Flow

- existing React Query hooks remain the data source
- page components only format and orchestrate existing API data
- mutations invalidate the relevant query keys
- errors stay local to the page or action that caused them
- no hidden stub flows or fake success states

### Frontend Error Interface

The frontend should not route on raw Zod parse failures. The shared client layer should normalize HTTP failures into a typed application error with at least:

- `status`
- `message`
- `kind`

Expected `kind` mapping for this redesign:

- `auth`
  - `401`
- `forbidden`
  - `403`
- `not_found`
  - `404`
  - current backend `400` responses that mean missing project or missing child resource, such as `unknown project`
- `failed`
  - all other unexpected request failures

Specific normalization rules for existing backend behavior:

- `400 unknown project`
  - `not_found`
- `400 unknown task`
  - `not_found`
- project file or file preview missing
  - `not_found`
- invalid filter input, unsupported media type, payload too large
  - `failed` with page-local messaging
- dead audit targets
  - do not raise route state in the audit list; render the audit row without a deep link

GET-driven pages should use this normalized error shape to drive the shared route-state matrix. Retry buttons are only shown for `failed` states, not for `auth`, `forbidden`, or `not_found`.

## Visual System

The design should use a compact, high-contrast internal-tool aesthetic:

- muted surfaces and thin borders
- clear typographic hierarchy
- status badges for queue, task, and review states
- card-based summaries for overview screens
- tables for list-heavy screens
- drawers or dialogs for destructive and multi-step actions
- sticky section headers where a page has many controls

The design should borrow the density and clarity of `llm_wiki`'s operational views, but remain a route-driven web admin.

## Navigation Model

### System Navigation

Keep the current system routes:

- Dashboard
- Projects
- Users
- Settings

### Project Navigation

Keep the current project routes and make them visually consistent:

- Overview
- Files
- Sources
- Source Watch
- Search
- Query
- Lint
- Graph
- Tasks
- Reviews
- Audit

The project overview page is the entry point for the project workbench.

## Screen Requirements

### Login

- centered card layout
- username and password inputs
- explicit error state for invalid credentials
- clear redirect on success

### Dashboard

- KPI cards must be limited to data already available from existing backend surfaces
- recent projects table or list
- quick links into Projects and Settings
- empty state when no projects exist yet

Dashboard data sources for this slice:

- `GET /api/projects`
  - project count
  - recent project list
- `GET /api/system/settings`
  - provider mode
  - language
  - default query limit
  - provider key configured state
- optional `GET /api/health`
  - backend reachability badge if a lightweight health card is implemented

Global task and review counts are out of scope for this redesign unless a backend aggregate endpoint already exists. The dashboard must not invent those metrics client-side.

### Projects

- shadcn table or card grid
- primary action to create a project
- project rows show name, root path, created time, and quick entry
- if a projectless state exists, it should be a full, intentional empty state with a primary CTA

Create-project flow requirements:

- trigger from a primary button in the page header or empty state
- collect `name` and `rootPath`
- validate both fields as trimmed non-empty values before submit
- keep form state open on validation or mutation failure
- surface backend errors inline in the dialog or sheet
- on success, invalidate the projects list and navigate to `/projects/:projectId`
- do not keep or reintroduce the current demo-project shortcut flow

### Project Overview

- project title, root path, and summary counts
- source watch status
- recent sources, tasks, reviews, and audit entries
- quick links into the project subpages

### Files

- filters for root, recursion, and max file count
- tree view in a scroll area
- preview panel with file content
- selected file state should be obvious

### Sources

- text import form
- file upload form
- folder upload form
- source table with ingest and delete actions
- rescan action with visible status message

### Source Watch

- form for enablement, path, include/exclude rules, file size, and interval
- auto-ingest toggle
- scan-now action
- last-scan metadata
- success and error messages surfaced inline

### Search

- query form with top-k and include-content toggle
- result summary chips for token and vector hits
- result cards with path links, snippets, image references, and optional content preview

### Query

- async task-oriented submission flow
- task polling with visible status
- rendered answer and citations
- save-to-wiki action
- retry and cancel controls when the task state allows them

### Lint

- structural and semantic lint triggers
- task status summary
- issue list with severity and file links

### Graph

- filter controls for query, node type, and limit
- node list in a scrollable area
- neighbor detail panel for the selected node

### Tasks

- task list with status filters
- task detail panel or drawer
- retry and cancel actions
- visible attempt counts and failure summaries

### Reviews

- unresolved/resolved/all filters
- type filter
- limit control
- review cards or table rows
- source and affected-page links
- resolve action and sweep action

### Audit

- chronological log table
- action, summary, target type, and target id when available
- easy navigation back to the affected project surface when the audit item has a routeable target

Audit route mapping rules:

- `targetType=project`
  - navigate to `/projects/:projectId`
- `targetType=source`
  - navigate to `/projects/:projectId/sources`
- `targetType=review`
  - navigate to `/projects/:projectId/reviews`
- `targetType=query`
  - navigate to `/projects/:projectId/query`
- `targetType=lint`
  - navigate to `/projects/:projectId/lint`
- `action` prefixed with `source.watch.`
  - navigate to `/projects/:projectId/source-watch`
- all other cases
  - keep the row read-only with no deep link

If the destination resource no longer exists, the target page falls back to the shared `not_found` or `failed` route state. The audit list itself remains navigable regardless of target validity.

### Users

- read-only user directory unless backend mutation endpoints are added later
- role badges and clean table presentation
- no filters, sorting, or edit controls are required in this slice
- if the backend returns an empty user list, show a simple empty state instead of an implied missing feature

### Settings

- system settings form with provider mode, language, query limit, base URL, API key, model, embedding model, and timeout
- API key must never be rendered back in plaintext
- API key input should load blank and show helper text equivalent to "leave blank to keep the current key"
- untouched blank and manually cleared blank both mean "preserve current key" in this phase
- entering a non-empty value means "replace current key"
- explicit key clearing or rotation UI is out of scope unless backend support is added later
- show configured vs. not configured state only
- save action should be explicit and visibly pending while saving

Settings request contract:

- if the API key field is blank at submit time, the client omits `providerApiKey` from the PATCH payload
- if the API key field is non-empty, the client sends `providerApiKey` with the replacement value
- the client must not send an empty string for `providerApiKey` in this redesign slice

## Shared Interaction Rules

- Use dialogs for destructive actions such as delete, cancel, and confirm-replace flows.
- Use badges for status values such as queued, running, failed, resolved, and enabled.
- Use skeletons for loading states rather than blank containers.
- Use inline alerts for errors that block the current action.
- Keep all forms controlled except native file inputs, and keep numeric validation local to the page.
- Preserve current route parameters when moving between project pages.

## Authentication

The admin app should remain session-aware:

- query `/api/auth/me` on startup
- redirect unauthenticated users to `/login`
- keep the CSRF token in session storage as the current implementation does

No new auth model is required for this redesign.

## Error Handling

The UI should distinguish:

- auth errors: redirect or block with a clear login message
- validation errors: show inline near the field
- mutation failures: keep the form state and show a visible error banner
- empty data: show an intentional empty state with a next action
- task failures: render the persisted task error, not a generic toast only
- read failures: show route-level error surfaces for forbidden, missing project, missing task, and unavailable data states
- stale project state: if the route refers to a project or child resource that no longer exists, show a not-found state with a way back to Projects

### Route/State Matrix

Use one shared route-state model across the project surfaces:

- `loading`
  - show skeletons or queued placeholders
- `ready`
  - render the route normally
- `empty`
  - show a next-step empty state
- `forbidden`
  - show access denied and route back to a safe page
- `not_found`
  - show missing project/resource state with a return action
- `failed`
  - show the page-specific error and retry action when available

Project-scoped pages should treat the current backend response `400` with message `unknown project` as `not_found` until the backend normalizes that case to `404`.

### Route State Vs Action State

Project pages with mutations or async tasks use two parallel state layers:

- route state
  - whether the page itself can render its underlying project/resource context
- action state
  - whether a user-triggered operation is idle, pending, succeeded, or failed

Rules:

- `Search`, `Query`, and `Lint` start in route state `ready` with action state `idle`
- a user can submit an action only when route state is `ready`
- action `pending` must not replace the full page with route-level loading
- action `failed` renders inline in the action/result area while the page stays in route state `ready`
- action `succeeded` renders the result area while the page stays in route state `ready`
- if the backing task lookup itself returns normalized `not_found`, the task/result panel shows a local not-found state while the route remains `ready`
- only failures that prevent the page from loading its base project/resource context may move the whole route into `forbidden`, `not_found`, or `failed`

## Testing Strategy

Keep tests narrow and focused on the critical path:

- login renders and submits
- auth guard redirects unauthenticated access to `/login`
- shell routes and project navigation render correctly
- one project operations smoke test covers the core action surfaces
- settings save flow persists values and keeps the API key masked
- one async task page test covers polling/result rendering
- one route-state regression test covers `403` forbidden and project-not-found handling

Avoid broad snapshot coverage and avoid testing trivial style details.

## Delivery Slices

Implementation should land in the following order:

1. admin UI foundation
   - shadcn-style primitives
   - shared utility helpers
   - shell and auth guard
2. system routes
   - dashboard, projects, users, settings
3. project workbench routes
   - overview, files, sources, source watch
4. operational routes
   - search, query, lint, graph, tasks, reviews, audit
5. polish
   - responsive behavior
   - empty/error/loading states
   - keyboard and dialog ergonomics

## Acceptance Criteria

The redesign is complete when:

- every current admin route renders with the new component system
- every current button/form action remains wired to a real hook or API call
- login still works and leads into the app shell
- project navigation is visually coherent and easy to use
- the settings page exposes the provider and system controls without revealing stored secrets
- the app feels like a genuine backend console rather than an empty scaffold
