# Team Layer (Org → Team → Member, Team-Owned Private KBs) — Design

**Date:** 2026-06-19
**Status:** Approved for planning
**Builds on:** `docs/superpowers/specs/2026-06-18-saas-multitenancy-design.md` and the
merged Phase 1 backend foundation (`migrations/0012_tenancy_foundation.sql`,
`src/tenancy/`).

## Goal

Add a **team layer** inside organizations so the structure becomes
**Org → Team → Member**. Teams own their own **private** knowledge bases (KBs);
org-level KBs are **public** to all org members. This extends the Phase 1 tenancy
model (personal / org spaces) by making a team a first-class **space kind**.

## Locked decisions (this brainstorming session)

| Topic | Decision |
|---|---|
| Structure | Org → Team → Member. Teams are **optional**; any org member can create a team and becomes its **leader**. |
| KB ownership | **Team owns its private KBs** (chosen approach: team is a third `spaces.kind`). A KB's `space_id` points to a personal / org / team space. |
| Org-level KBs | "Public projects": created by `org_admin` at the org level; **every org member gets `viewer` by default**. |
| Team-level KBs | Private to the team. Leader manages per-member grants; members have **no access** until explicitly granted. |
| Public default role | `viewer` (read / search / query / chat). Editor must be granted. |
| Leader scope | Leader manages member grants **only on their own team's KBs**; default content role `editor`; may create/delete the team's KBs and add/remove the team's members. **Single leader** (the creator); no multi-leader / transfer for now. |
| Who creates public KBs | **`org_admin` only.** |
| org_admin | Always `Owner` on **all** org KBs, including team-private KBs. |
| Semantic change | Phase 1 treated org-space KBs as grant-gated (no access without a grant). **This changes:** org-space KB = public (all members `viewer`); the team space is now the private container. Phase 1 tests are updated accordingly. |
| Scope | **Full feature incl. UI**, delivered as three sequential plans (see Implementation Phasing). |

## Why approach C (team = space kind)

A KB already belongs to exactly one `spaces` row via `projects.space_id`. Making a
team a third space kind keeps **one ownership path**: the space tells you
everything (personal vs public-org vs team-private). The alternative of a
`projects.team_id` derived column or a `project_team_grants` table either
duplicates ownership state or conflicts with the "members default to no access"
rule. The cost is a one-time `spaces` constraint widening (below).

## Data model

### New tables

```sql
teams (
  id         TEXT PRIMARY KEY NOT NULL,
  org_id     TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  name       TEXT NOT NULL,
  slug       TEXT NOT NULL,                       -- URL-safe handle, unique within org
  created_by TEXT NOT NULL REFERENCES users(id)  ON DELETE CASCADE,
  created_at TEXT NOT NULL,                        -- RFC3339, matches existing convention
  UNIQUE(org_id, slug)
)

team_members (
  id         TEXT PRIMARY KEY NOT NULL,
  team_id    TEXT NOT NULL REFERENCES teams(id)  ON DELETE CASCADE,
  user_id    TEXT NOT NULL REFERENCES users(id)  ON DELETE CASCADE,
  role       TEXT NOT NULL CHECK (role IN ('leader','member')),
  created_at TEXT NOT NULL,
  UNIQUE(team_id, user_id)
)
```

### Changed table: `spaces`

`migrations/0012` created `spaces` with two inline, unnamed CHECKs. Postgres
auto-names them `spaces_kind_check` (the `kind` column check) and `spaces_check`
(the table-level composite). Migration 0013 widens them:

```sql
ALTER TABLE spaces ADD COLUMN team_id TEXT REFERENCES teams(id) ON DELETE CASCADE;

ALTER TABLE spaces DROP CONSTRAINT spaces_kind_check;  -- old: kind IN ('personal','org')
ALTER TABLE spaces DROP CONSTRAINT spaces_check;       -- old: 2-way composite

ALTER TABLE spaces ADD CONSTRAINT spaces_kind_check
  CHECK (kind IN ('personal','org','team'));

ALTER TABLE spaces ADD CONSTRAINT spaces_owner_check CHECK (
  (kind='personal' AND owner_user_id IS NOT NULL AND org_id IS NULL  AND team_id IS NULL) OR
  (kind='org'      AND org_id        IS NOT NULL AND owner_user_id IS NULL AND team_id IS NULL) OR
  (kind='team'     AND team_id       IS NOT NULL AND owner_user_id IS NULL AND org_id IS NULL)
);

CREATE UNIQUE INDEX spaces_team ON spaces(team_id) WHERE kind='team';
```

> The plan's first step verifies the two constraint names with `\d spaces` against
> a freshly migrated DB before relying on the `DROP CONSTRAINT` calls.

### Unchanged

- `projects.space_id` (now may point at a `team` space). No new column on `projects`.
- Per-KB grants continue to use **`project_members(project_id, user_id, role, can_import)`**;
  no new grant table. `role ∈ {owner, editor, viewer}`, `can_import` derived
  (owner/editor → true, viewer → false).

## Access control

Single resolver `tenancy::access::project_access_role(pool, project_id, user_id)`
returns `Option<AccessRole>` (`Owner | Editor | Viewer`). Extended branches:

```
space = spaces[project.space_id]            # (kind, owner_user_id, org_id, team_id)

personal:
    owner_user_id == user_id ? Owner : None                         # unchanged

org (PUBLIC KB):
    m = organization_members[org_id, user_id]
    m is None                       -> None                          # non-member
    m.role == 'org_admin'           -> Owner
    else (org_member):
        g = project_members[project, user_id]
        g in {owner, editor}        -> Editor
        else                        -> Viewer                        # ★ public default

team (PRIVATE KB):
    org_id = teams[space.team_id].org_id
    organization_members[org_id, user_id].role == 'org_admin' -> Owner
    tm = team_members[space.team_id, user_id]
    tm.role == 'leader'             -> Editor
    g = project_members[project, user_id]
    g in {owner, editor}            -> Editor
    g == 'viewer'                   -> Viewer
    else                            -> None                          # member w/o grant, or outsider
```

API tokens are unchanged: project-scoped, `token.project_id` must equal the
project; CSRF still applies to session principals only.

### Management capability (new helper)

For the grant/management endpoints (plan 2). Distinct from content role because a
leader's content role is `editor` yet they may administer grants on their team's KBs:

```
can_manage_kb_access(pool, project_id, user_id) -> bool =
    space = spaces[project.space_id]
    org_admin of the owning org                              -> true
    space.kind == 'team' AND leader of space.team_id         -> true
    otherwise                                                -> false
```

### Capabilities by effective role

| Capability | viewer | editor | leader (own team KB) | owner / org_admin |
|---|---|---|---|---|
| Read wiki / search / query / chat | ✓ | ✓ | ✓ | ✓ |
| Import sources / edit wiki / ingest | | ✓ | ✓ | ✓ |
| Manage members & per-KB grants | | | ✓ (own team) | ✓ |
| Create / delete KB | | | ✓ (own team) | ✓ |
| Create **public** org KB | | | | ✓ (org_admin) |

Plus: **any org member** may create a team and becomes its leader (independent of
any KB role).

## Endpoints (plan 2)

All session-authenticated + CSRF. Org-management endpoints below fill the gap left
by Phase 1 (none exist today); teams require them.

- `POST   /api/orgs` — create org; creator inserted as `org_admin`; also creates the
  org's `spaces(kind='org')` row. (Any authenticated user.)
- `GET    /api/spaces` — list contexts available to the caller for the switcher:
  personal space + each org (with the caller's org role) + teams the caller belongs to.
- `POST   /api/orgs/{org}/members` — add an **existing** user by username; body
  `{ usernameOrEmail, role }`; `org_admin` only; clear error if user not found.
- `DELETE /api/orgs/{org}/members/{userId}` — `org_admin` only.
- `POST   /api/orgs/{org}/teams` — create team; creates `teams` row +
  `spaces(kind='team')` row + `team_members(leader)`; any org member; slug unique per org.
- `GET    /api/orgs/{org}/teams` — teams the caller can see (member-of, or all if org_admin).
- `POST   /api/orgs/{org}/teams/{team}/members` — add team member (existing org member);
  `org_admin` or the team leader.
- `DELETE /api/orgs/{org}/teams/{team}/members/{userId}` — `org_admin` or leader.
- `POST   /api/projects` (extended) — accept a target `spaceId`; gate creation:
  org space → `org_admin`; team space → leader/member of that team; personal → self.
- `GET    /api/projects` (extended) — scope to the active space; for an org context
  returns public KBs the caller can see + team KBs the caller can access.
- `POST   /api/projects/{id}/grants` — upsert a per-KB grant `{ userId, role }`;
  gated by `can_manage_kb_access`; target must be a member of the owning org (and,
  for a team KB, a member of the owning team).
- `DELETE /api/projects/{id}/grants/{userId}` — same gate.
- `DELETE /api/projects/{id}` (existing, re-gated) — allowed for `org_admin` and, for a
  team KB, the team leader (per the capabilities table), in addition to the current
  `Owner` check.

Zod schemas for every new request/response go in
`packages/api-client/src/schemas.ts`.

## Settings interaction

Team KBs resolve provider/settings via their **owning org → instance default**
(the org-level resolver from the master design). **No team-level settings
override** — teams inherit the org's config. This keeps the resolver to two
tenant layers (org, user) plus the instance default.

## Admin UI (plan 3)

`apps/admin` has **no tenancy surface today**, so this plan introduces it.

**New**
- **Space switcher** in the top nav: `Personal` + each org. Selecting an org enters
  that org's workspace.
- **Org workspace KB list**, grouped: **公共项目** (public) section + one section per
  team the caller can access. Only KBs the caller can see are listed.
- **Org admin:** "新建公共项目" action; org members page (list / add by username /
  remove / set role).
- **Any org member:** "新建团队" action; a team page showing its members and KBs.
- **Team leader (on own team):** add/remove team members; "新建团队库"; per-KB
  "管理访问" to grant `editor`/`viewer` to team members.
- **Per-KB "管理访问"** dialog for `org_admin` (public + team KBs) and leaders (own
  team KBs).

**Changed**
- Projects list scoped to the active space (Personal vs an org workspace).

**Unchanged**
- Every per-project screen (files, sources, search, query, chat, graph, tasks,
  reviews, dedup, …).

## Migration

One migration, `migrations/0013_team_layer.sql`:

1. `CREATE TABLE teams`, `CREATE TABLE team_members`.
2. Widen `spaces`: add `team_id`, drop+recreate `spaces_kind_check` and
   `spaces_check`, add `spaces_team` unique index (SQL above).
3. **No backfill** — there are no existing teams; existing personal/org spaces and
   all current projects are untouched.

Pre-production: schema is changed directly; no back-compat shims.

## Testing

Extend `tests/rust-integration/tests/tenancy_access_api.rs` (2-space indent, per the
existing file):

- **Migration:** `teams` / `team_members` exist; `spaces` accepts `kind='team'`;
  `spaces_team` enforces one space per team; the 3-way `spaces_owner_check` rejects
  a malformed team space (e.g. `kind='team'` with `org_id` set).
- **Resolver matrix:**
  - org public KB: non-member → None; org_member (no grant) → **Viewer**; org_member
    with editor grant → Editor; org_admin → Owner.
  - team KB: org_admin → Owner; leader → Editor; team member with editor/viewer
    grant → Editor/Viewer; team member without grant → None; org member not in the
    team → None; non-member → None.
- **`can_manage_kb_access`:** org_admin → true (public + team); leader → true on own
  team KB, false on another team's KB; granted editor (non-leader) → false; viewer → false.
- **Update existing `access_role_matrix`** expectation: org member without a grant on
  an org-space KB now resolves to **Viewer** (was None).

Endpoint tests (plan 2) and component/e2e tests (plan 3) are specified in their
respective plans.

## Implementation phasing (sequential, each independently mergeable)

1. **Backend foundation** — migration 0013, resolver branches, `can_manage_kb_access`,
   integration tests + the Phase 1 test update. No HTTP endpoints.
2. **Backend endpoints** — the org/team/grant/project endpoints above + Zod schemas +
   endpoint integration tests.
3. **Frontend** — space switcher, grouped org KB list, team + member management, grant
   UI, create-KB flows + component/e2e tests.

Each phase gets its own plan via `writing-plans`; brainstorming proceeds to plan 1 first.

## Out of scope

- Billing, plans, quotas, metering.
- Email invitations (members are added from existing registered users) and
  self-service signup (tracked by the master tenancy design, not this feature).
- Team-level provider/settings overrides (teams inherit org → instance).
- Multiple leaders per team / leadership transfer / nested (sub-)teams.
- Sharing a **personal** KB with other users (personal stays private; sharing is via
  org public KBs or team KBs).
