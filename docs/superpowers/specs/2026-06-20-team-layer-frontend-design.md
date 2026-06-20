# Team Layer — Frontend (Admin UI, Plan 3) Design

**Date:** 2026-06-20
**Status:** Approved for planning
**Builds on:** `docs/superpowers/specs/2026-06-19-team-layer-design.md` (§"Admin UI (plan 3)"),
the merged Plan 1 (backend foundation) and Plan 2 (backend endpoints + Zod schemas).

## Goal

Introduce the tenancy surface in `apps/admin`: a space switcher, a space-scoped /
grouped KB list, org + team member management, per-KB access grants, and the
create-KB flows — delivered as **one plan**. The plan also adds the few **additive
backend read/update endpoints** the UI needs but Plan 2 did not provide.

## Locked decisions (this brainstorming session)

| Topic | Decision |
|---|---|
| Interface language | **Hardcoded English** for the new tenancy UI, consistent with the existing `apps/admin` (no i18n framework exists). The spec's Chinese labels are translated: 公共项目 → "Public projects", 新建公共项目 → "New public project", 新建团队 → "New team", 新建团队库 → "New team KB", 管理访问 → "Manage access". |
| Plan granularity | **One complete plan** covering the whole "Admin UI (plan 3)" section. |
| Navigation model | **Real routes.** Org/team are route segments (`/orgs/:orgId`, `/orgs/:orgId/members`, `/orgs/:orgId/teams/:teamId`); Personal stays at the existing projects route. Active space = current route (deep-linkable, refresh/back-safe). Mirrors the backend resource hierarchy; each surface is an independently testable page. |
| Org workspace layout | **Stacked sections.** One page shows a "Public projects" section then one section per team the caller can access, each with its own actions. Team pages still exist as deep routes for member management. |
| Backend additions | The "frontend" plan **includes** the small additive endpoints required to list members and set roles (see §"Additive backend"). No upstream to port — `upstream_llm_wiki` is a single-user desktop app with no tenancy concept. |

## Existing surface this plan extends

Frontend (`apps/admin`, React 19 / Vite / react-router-dom 7 / @tanstack/react-query 5 /
shadcn-ui / Tailwind 4 / Zod 4 / `@knowledge/api-client`):

- `src/app/router.tsx` — route tree; tenancy routes mount under `AppShell`.
- `src/components/layout/top-bar.tsx` — currently a static header; gains the space switcher.
- `src/features/auth/use-session.ts` — `useSession()` → `GET /api/auth/me` → `{user:{id,username,role}}`.
- `src/features/shared/api.ts` — API functions + `csrfHeader()` (CSRF token in
  `sessionStorage["knowledge.csrfToken"]`).
- `src/features/projects/{page,queries,mutations,detail-page}.tsx` — closest templates;
  `apiFetch(path, init, schema)` from `@knowledge/api-client`; create-dialog + inline-error
  pattern in `projects/page.tsx`.
- `packages/api-client/src/schemas.ts` — already exports `parseSpaceList`, `parseProjectList`
  (`scopedProjectSchema`: `id, name, rootPath, createdAt, spaceKind:"personal"|"org"|"team",
  teamId:string|null, teamSlug:string|null, role:"owner"|"editor"|"viewer"`), `parseTeamList`,
  `parseGrant`, `parseCurrentUser`.

Backend endpoints already available (verified in `crates/knowledge-server/src`):

| Method & path | Response (camelCase) | Gate |
|---|---|---|
| `GET /api/spaces` | `{personal:{spaceId\|null}, orgs:[{id,slug,name,spaceId,role:"org_admin"\|"org_member"}], teams:[{id,orgId,slug,name,spaceId,role:"leader"\|"member"}]}` | session |
| `POST /api/orgs` | `201 {id,name,slug,spaceId}` | any session + CSRF |
| `POST /api/orgs/{org}/members` | `201 {orgId,userId,role}` (body `{usernameOrEmail,role}`) | org_admin + CSRF |
| `DELETE /api/orgs/{org}/members/{userId}` | `204` | org_admin + CSRF |
| `GET /api/orgs/{org}/teams` | `{teams:[{id,name,slug,orgId}]}` (admin: all; member: own) | org member |
| `POST /api/orgs/{org}/teams` | `201 {id,name,slug,orgId,spaceId}` (body `{name,slug}`) | org member + CSRF |
| `POST /api/orgs/{org}/teams/{team}/members` | `201 {teamId,userId,role:"member"}` (body `{usernameOrEmail}`) | org_admin or leader + CSRF |
| `DELETE /api/orgs/{org}/teams/{team}/members/{userId}` | `204` (cannot remove leader) | org_admin or leader + CSRF |
| `GET /api/projects?space_id=X` | `{projects:[scopedProject]}` (omitted → personal) | space-scoped membership |
| `POST /api/projects` | `201 {id,name,rootPath,createdAt}` (body `{name, spaceId?}`) | space-gated + CSRF |
| `DELETE /api/projects/{id}` | gated (Owner / `can_manage_kb_access`) | + CSRF |
| `GET /api/projects/{id}/members` | `{members:[{userId,role,canImport}]}` → **enriched with `username`** below | session w/ project access |
| `POST /api/projects/{id}/grants` | `200 {projectId,userId,role,canImport}` (body `{userId,role:"editor"\|"viewer"}`) | `can_manage_kb_access` + CSRF |
| `DELETE /api/projects/{id}/grants/{userId}` | `204` | `can_manage_kb_access` + CSRF |
| `GET /api/users` | `{users:[{id,username,role}]}` | (existing; used only as a username source where needed) |

## Additive backend (part of this plan)

Small, additive endpoints in the existing `crates/knowledge-server/src/tenancy/` modules
(4-space indent; SQLx runtime string queries with `.bind()`; `authorized_principal` /
`validate_csrf` for mutations; helpers `is_org_admin`, `org_member_role`,
`team_member_role`). Each gets a `rust-integration` test mirroring
`tests/rust-integration/tests/team_endpoints_api.rs`.

1. **`GET /api/orgs/{org}/members`** (in `tenancy/orgs.rs`)
   → `{members:[{userId,username,role}]}`, ordered by `organization_members.created_at ASC`.
   Gate: caller is an org member (`org_member_role(...).is_some()`) else 403. SQL joins
   `organization_members` → `users` for `username`.

2. **`PATCH /api/orgs/{org}/members/{userId}`** (in `tenancy/orgs.rs`)
   Body `{role}` (`role ∈ {"org_admin","org_member"}`, else 400). Gate: `is_org_admin` + CSRF
   (else 403). Updates `organization_members.role`. 404 if the membership row does not exist
   (`UPDATE ... ` then check `rows_affected`). Returns `200 {orgId,userId,role}`. No
   "last admin" guard (mirrors the existing `DELETE member` behavior; noted as a known
   limitation, consistent with current code).

3. **`GET /api/orgs/{org}/teams/{team}/members`** (in `tenancy/teams.rs`)
   → `{members:[{userId,username,role}]}`, ordered by `team_members.created_at ASC`.
   Gate: caller is `is_org_admin` **or** a member of the team (`team_member_role(...).is_some()`)
   else 403. Verifies the team belongs to the org (404 otherwise). SQL joins
   `team_members` → `users`.

4. **Enrich `GET /api/projects/{id}/members`** (in `projects/routes.rs`, `list_project_members`)
   Add `username` to each row via `JOIN users u ON u.id = pm.user_id`. Response becomes
   `{members:[{userId,username,role,canImport}]}`. Additive field; existing callers unaffected.

New Zod schemas in `packages/api-client/src/schemas.ts` + tests in `schemas.test.ts`:
- `orgMemberListSchema` / `parseOrgMemberList` → `{members:[{userId,username,role:"org_admin"|"org_member"}]}`
- `teamMemberListSchema` / `parseTeamMemberList` → `{members:[{userId,username,role:"leader"|"member"}]}`
- `projectMemberListSchema` / `parseProjectMemberList` → `{members:[{userId,username,role:"owner"|"editor"|"viewer",canImport:boolean}]}`

## Frontend architecture

New feature folders under `apps/admin/src/features/`, each following the
`page.tsx / queries.ts / mutations.ts / page.test.tsx` convention and using
`apiFetch` + the api-client schemas:

```
features/
  spaces/
    use-spaces.ts          # useSpacesQuery() -> GET /api/spaces (queryKey ["spaces"])
    space-switcher.tsx     # top-nav dropdown: Personal + each org; navigates
    space-switcher.test.tsx
  orgs/
    workspace-page.tsx     # /orgs/:orgId  (stacked grouped KB list + actions)
    workspace-queries.ts   # scoped projects + teams
    members-page.tsx       # /orgs/:orgId/members
    members-queries.ts
    members-mutations.ts   # add / remove / set-role
    create-org-dialog.tsx  # "New org" (entered from the switcher)
    create-public-project-dialog.tsx
    create-team-dialog.tsx
    *.test.tsx
  teams/
    team-page.tsx          # /orgs/:orgId/teams/:teamId  (members + team KBs)
    team-queries.ts
    team-mutations.ts      # add / remove team member; new team KB
    *.test.tsx
  kb-access/
    manage-access-dialog.tsx   # per-KB grant editor (list/add/remove)
    manage-access-queries.ts   # GET /api/projects/:id/members
    manage-access-mutations.ts # upsert / delete grant
    *.test.tsx
```

Shared API functions are added to `features/shared/api.ts` (or a new
`features/shared/tenancy-api.ts` if `api.ts` is already large) using `apiFetch`,
`csrfHeader()`, and the new schemas.

### Routing (`src/app/router.tsx`, under `AppShell`)

```
<Route path="orgs/:orgId" element={<OrgWorkspacePage />} />
<Route path="orgs/:orgId/members" element={<OrgMembersPage />} />
<Route path="orgs/:orgId/teams/:teamId" element={<TeamPage />} />
```

Personal space keeps the existing `projects` route. Each tenancy page reads its ids
from `useParams()` and resolves the caller's role from `useSpacesQuery()`.

### Space switcher (top bar)

`top-bar.tsx` renders `<SpaceSwitcher />`. Options: **Personal** + each org from
`GET /api/spaces`. Selecting Personal → navigate `/projects`; selecting an org →
navigate `/orgs/:orgId`. The current selection is derived from the URL (`useParams` /
`useLocation`), so refresh and deep links are correct without extra persistence. A
"New org" item opens `create-org-dialog` (`POST /api/orgs`), then navigates to the new
org on success.

### Org workspace (`/orgs/:orgId`) — stacked sections

Two queries:
- `GET /api/projects?space_id=<org.spaceId>` → flat scoped list. Group client-side:
  `spaceKind === "org"` → **Public projects** section; `spaceKind === "team"` → group by
  `teamId` (label from `teamSlug`/team name).
- `GET /api/orgs/:orgId/teams` → full set of teams the caller can see, so teams **with no
  accessible KB** still render a section, and per-team "New team KB" actions appear.

Role (from `useSpacesQuery`):
- **org_admin** (`orgs[].role === "org_admin"`): header actions "New public project"
  (`POST /api/projects {name, spaceId: org.spaceId}`), "Members" (link to
  `/orgs/:orgId/members`), "New team" (`create-team-dialog`). Every KB row shows
  "Manage access".
- **team leader** (a `teams[]` entry with `role === "leader"` and matching `orgId`):
  on their own team's section, "New team KB" (`POST /api/projects {name, spaceId: team.spaceId}`)
  and "Manage access" per KB; "Manage" links to the team page.
- **org_member**: read-only grouped list; "New team" is available to **any** org member.

Each KB row links into the existing per-project workspace (`/projects/:projectId`).

### Org members page (`/orgs/:orgId/members`)

`GET /api/orgs/:orgId/members` → table (username, role). Visible to any org member.
Mutations gated to **org_admin** (hidden otherwise; backend enforces):
- Add by username: input + role select → `POST /api/orgs/:orgId/members {usernameOrEmail, role}`.
- Set role: per-row role select → `PATCH /api/orgs/:orgId/members/:userId {role}`.
- Remove: per-row → `DELETE /api/orgs/:orgId/members/:userId`.

### Team page (`/orgs/:orgId/teams/:teamId`)

- Members: `GET /api/orgs/:orgId/teams/:teamId/members` → table (username, role).
  Add (`POST .../members {usernameOrEmail}`) / Remove (`DELETE .../members/:userId`) for
  **org_admin or the team leader**; leader row not removable (backend enforces).
- Team KBs: filter the org-scoped projects query (or re-query the team space) for this
  `teamId`; "New team KB" (`POST /api/projects {name, spaceId: team.spaceId}`); per-KB
  "Manage access".

### Manage-access dialog (per KB)

Triggers from any KB row whose caller can manage it (org_admin, or leader of the owning
team). Contents:
- Current grantees: `GET /api/projects/:id/members` → list `username` + role; Remove →
  `DELETE /api/projects/:id/grants/:userId`.
- Add / change grant: pick a candidate (org members for a public KB; team members for a
  team KB — from the relevant members query) + role (editor/viewer) →
  `POST /api/projects/:id/grants {userId, role}`.

## Data flow & caching

- React Query keys: `["spaces"]`; `["org-projects", orgSpaceId]`; `["org-teams", orgId]`;
  `["org-members", orgId]`; `["team-members", orgId, teamId]`; `["project-grants", projectId]`.
  Including the space/org/team id in the key isolates caches across spaces.
- Mutations invalidate the relevant keys on success (e.g. create public project →
  invalidate `["org-projects", orgSpaceId]`; add org member → `["org-members", orgId]`;
  upsert grant → `["project-grants", projectId]`).
- CSRF: all mutations send `csrfHeader()` (token from `sessionStorage["knowledge.csrfToken"]`),
  matching existing mutations.

## Error handling & edge cases

- Capability-gated actions are **not rendered** when the caller lacks the role (derived
  from `/api/spaces`); the backend still enforces with 401/403 as the source of truth.
- Backend 400/404 (slug taken, user not found, already a member, not an org/team member)
  surface as **inline errors** inside the originating dialog (reuse the
  `projects/page.tsx` create-dialog error pattern), not as crashes.
- Empty states: org with no public KBs / no teams; team with no members or no KBs — each
  section shows a short empty message and the relevant create action when permitted.
- A non-member hitting an org/team route gets the backend 403; the page renders an
  "access denied / not found" state rather than a blank screen.
- Switching space never leaks another space's data because cache keys are space-scoped.

## Testing

**Frontend** — `vitest` + `@testing-library/react`, wrapped in `MemoryRouter` +
`QueryClientProvider`, mocking the network at `apiFetch`/`fetch`. Per surface:
- Space switcher: lists Personal + orgs from `/api/spaces`; selecting navigates to the
  right route; "New org" opens the dialog.
- Org workspace: groups a mixed scoped-projects payload into Public + per-team sections;
  shows admin-only vs leader-only vs member actions per role; empty team still renders.
- Org members page: renders members; admin sees add/remove/set-role; non-admin does not.
- Team page: renders members + team KBs; leader/admin controls; leader row not removable.
- Manage-access dialog: lists grantees with usernames; add/remove grant calls the right
  endpoints and invalidates the cache.

**Backend** — `rust-integration` tests for the four additive items, mirroring
`team_endpoints_api.rs` (seeded admin login; `--test-threads=1`): list org members
(member ok, non-member 403, usernames present); patch org member role (admin ok, member
403, bad role 400, missing membership 404); list team members (admin/team-member ok,
outsider 403); enriched project-members payload includes `username`.

## File structure summary

| Area | Create | Modify |
|---|---|---|
| Routing | — | `apps/admin/src/app/router.tsx` |
| Top nav | `features/spaces/{use-spaces.ts, space-switcher.tsx, *.test.tsx}` | `components/layout/top-bar.tsx` |
| Org workspace | `features/orgs/{workspace-page,workspace-queries,create-public-project-dialog,create-team-dialog,create-org-dialog}.tsx + tests` | — |
| Org members | `features/orgs/{members-page,members-queries,members-mutations}.tsx + tests` | — |
| Team page | `features/teams/{team-page,team-queries,team-mutations}.tsx + tests` | — |
| KB access | `features/kb-access/{manage-access-dialog,manage-access-queries,manage-access-mutations}.tsx + tests` | — |
| Shared API | (new tenancy api fns) | `features/shared/api.ts` |
| Schemas | — | `packages/api-client/src/{schemas.ts, schemas.test.ts}` |
| Additive backend | `rust-integration` tests | `crates/knowledge-server/src/tenancy/{orgs.rs,teams.rs}`, `crates/knowledge-server/src/projects/routes.rs` |

## Out of scope

- Anything in the parent spec's "Out of scope" (billing, invitations/self-signup,
  team-level settings, multi-leader, personal-KB sharing).
- Changes to per-project screens (files/sources/search/query/chat/graph/tasks/reviews/dedup) —
  unchanged.
- i18n framework; UI stays hardcoded English.
- "Last org_admin" demotion/removal guard (mirrors current backend behavior).
