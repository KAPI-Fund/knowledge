# SaaS Multi-Tenancy (Organizations, Personal Spaces, Per-KB Access) — Design

**Date:** 2026-06-18
**Status:** Approved for planning

## Goal

Replace the current "one admin owns all knowledge bases" model with a SaaS-style
multi-tenancy model: anyone can self-register, every user gets a **personal
space**, and any user can create/join **organizations**. A knowledge base (KB =
"project") belongs to exactly one owner — a personal space or an org. Org admins
manage all of their org's KBs and authorize members to access specific KBs with a
per-KB role. The settings page is split into instance / org / personal scopes.

## Background (validated against current code)

The system is **not** "single admin manages all" at the schema level — the
foundation for membership-gated access already exists:

1. **`project_members` already gates project access.**
   `crates/knowledge-server/src/projects/routes.rs:1455-1482`
   `authorized_principal()` resolves the principal, checks token scope, then runs
   `SELECT COUNT(*) FROM project_members WHERE project_id=$1 AND user_id=$2` and
   returns 403 on 0. The table (`migrations/0001_init.sql:42-52`) carries
   `role` (currently only `'project_owner'`) and `can_import BOOLEAN`,
   `UNIQUE(project_id, user_id)`.

2. **Project creation already assigns an owner.**
   `crates/knowledge-server/src/projects/service.rs:23-83` auto-inserts the
   creator into `project_members` as `project_owner` with `can_import=true`.

3. **Auth is session + CSRF + API-token based.** `auth/principal.rs:14-40`
   `Principal { user_id, csrf_token, scope (Session|ApiToken), token_id,
   project_id }` with `permits_project()`. CSRF (`validate_csrf`,
   `projects/routes.rs:1364-1383`) applies to session principals only; API tokens
   are project-scoped and skip CSRF.

4. **`users.role` exists but is not enforced.** All authenticated users are
   treated equally today. `migrations/0001_init.sql:1-7`. The seeded admin
   (`lib.rs:132-167 seed_admin_user()`) hardcodes `role="admin"`.

5. **`system_settings` is a global singleton** (`CHECK id=1`): provider mode,
   base URL, API key, model, embedding model, timeouts, search provider/keys,
   language, default query limit. Instance-wide, no per-tenant scoping.

6. **No tenancy surface in the UI.** `apps/admin/src/app/router.tsx` has
   `/login`, `/projects`, `/projects/:projectId/*`, read-only `/users`,
   `/api-tokens`, global `/settings`. No org/tenant nav, no signup, no
   member-management UI, no per-project settings.

This redesign **extends** the existing per-KB ACL rather than replacing it.

## Upstream alignment note

`upstream_llm_wiki/` is a **single-user desktop (Tauri) app with no
multi-tenancy concept**. There is therefore **no upstream code to port** for the
tenancy / authorization / settings-scope layer — it is necessarily net-new
server design. All KB *internals* (ingest pipeline, knowledge graph, query /
retrieval, wiki structure, document parsing) are untouched by this work and
remain upstream-aligned. Multi-tenancy is a thin wrapper around the existing
per-project logic.

## Decisions (locked during brainstorming)

| Topic | Decision |
|-------|----------|
| Tenancy model | Personal space + optional organizations (GitHub-style). 1 personal space + N org memberships per user. |
| Signup | Open self-service signup. Instance operator = ops only, not involved in tenant signup/KB management. |
| Roles | Org roles `org_admin` / `org_member`; per-KB roles `owner` / `editor` / `viewer`. Members see nothing until granted a specific KB. |
| Settings scope | Hybrid: instance default provider config + optional per-org and per-personal overrides, resolved field-by-field. |
| Scope | Structure only — no billing, plans, quotas, or metering. |
| Migration | Seeded admin becomes a normal user (instance operator flag); all existing projects become that user's personal KBs; can be moved into an org later. |
| Ownership model | Unified `spaces` / namespaces table. One ownership path; "move KB to org" = change `space_id`. |

## Data model

### New tables

```sql
organizations(
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  slug         TEXT NOT NULL UNIQUE,        -- URL-safe handle
  created_by   TEXT NOT NULL REFERENCES users(id),
  created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
)

organization_members(
  id        TEXT PRIMARY KEY,
  org_id    TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  user_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role      TEXT NOT NULL CHECK (role IN ('org_admin','org_member')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(org_id, user_id)
)

spaces(
  id            TEXT PRIMARY KEY,
  kind          TEXT NOT NULL CHECK (kind IN ('personal','org')),
  owner_user_id TEXT REFERENCES users(id) ON DELETE CASCADE,        -- set iff kind='personal'
  org_id        TEXT REFERENCES organizations(id) ON DELETE CASCADE,-- set iff kind='org'
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK ( (kind='personal' AND owner_user_id IS NOT NULL AND org_id IS NULL)
       OR (kind='org'      AND org_id IS NOT NULL AND owner_user_id IS NULL) )
)
-- one personal space per user, one space per org:
CREATE UNIQUE INDEX spaces_personal_owner ON spaces(owner_user_id) WHERE kind='personal';
CREATE UNIQUE INDEX spaces_org            ON spaces(org_id)        WHERE kind='org';

organization_settings(
  org_id              TEXT PRIMARY KEY REFERENCES organizations(id) ON DELETE CASCADE,
  -- all provider fields NULLable; NULL = inherit instance default
  provider_mode       TEXT,
  provider_base_url   TEXT,
  provider_api_key    TEXT,
  provider_model      TEXT,
  embedding_model     TEXT,
  timeout_seconds     INTEGER,
  search_provider     TEXT,
  search_api_key      TEXT,
  language            TEXT,
  default_query_limit INTEGER
)

user_settings(
  user_id             TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  -- same NULLable provider fields as organization_settings
  provider_mode       TEXT,
  provider_base_url   TEXT,
  provider_api_key    TEXT,
  provider_model      TEXT,
  embedding_model     TEXT,
  timeout_seconds     INTEGER,
  search_provider     TEXT,
  search_api_key      TEXT,
  language            TEXT,
  default_query_limit INTEGER
)
```

> The exact column set of `organization_settings` / `user_settings` mirrors the
> overridable subset of `system_settings`; the implementation plan will copy the
> precise column list from `migrations` for `system_settings` so the three stay
> in sync.

### Changed tables

```sql
-- projects: tie each KB to exactly one owning space
ALTER TABLE projects ADD COLUMN space_id TEXT REFERENCES spaces(id);
-- (NOT NULL enforced after backfill; see Migration)

-- project_members: repurpose as the per-KB grant
--   role:  project_owner -> owner | editor | viewer
--   can_import derived from role (owner/editor = true, viewer = false)

-- users: role becomes instance-operator flag
--   role:  admin -> operator ; everyone else -> user
```

## Access control

Single authorization path, replacing the bare `project_members` count check at
`projects/routes.rs:1455-1482`:

```
authorize(requester, project):
  space = spaces[project.space_id]

  if space.kind == 'personal':
      allow iff space.owner_user_id == requester.user_id        # role = owner
      else 403

  if space.kind == 'org':
      m = organization_members[space.org_id, requester.user_id]
      if m is None:                          403                 # not a member
      if m.role == 'org_admin':              allow (role = owner / full)
      g = project_members[project.id, requester.user_id]
      if g is None:                          403                 # member, no grant
      else:                                  allow (role = g.role: viewer|editor)

  # API tokens unchanged: project-scoped, token.project_id must equal project.id
```

Capabilities by effective role:

| Capability | viewer | editor | owner / org_admin |
|---|---|---|---|
| Read wiki / search / query / chat | ✓ | ✓ | ✓ |
| Import sources / edit wiki / ingest | | ✓ | ✓ |
| Manage members & per-KB grants | | | ✓ |
| Delete KB / move KB to org | | | ✓ |

- Org members see **no** KBs until granted one — this is the literal
  "授权成员访问某个知识库".
- CSRF behavior unchanged (session principals only); API tokens still skip CSRF.

## Signup & onboarding

- **`POST /api/auth/signup`** (public): username/email + password → create `users`
  row + auto-create the user's personal `spaces` row + establish a session.
  Login flow otherwise unchanged.
- **Space switcher**: after login the active context is Personal · Org A · Org B…
  The projects list and all project routes are scoped to the active space.
- **`POST /api/orgs`**: create `organizations` + its `spaces(kind='org')` row +
  insert creator into `organization_members` as `org_admin`.
- **Adding members** (consistent with the open-signup choice — *no* email-invite
  infrastructure): an org admin adds an **existing registered user** by
  username/email. If the person has no account, they self-sign-up first, then the
  admin adds them. Endpoint: `POST /api/orgs/{org}/members { usernameOrEmail, role }`;
  returns a clear error if the user is not found.
- **Instance operator**: the seeded admin's role becomes `operator`. Operator
  scope is ops-only — instance health, the global default provider config, and
  the user list. The operator is **not** auto-added to tenants and does not
  manage tenant KBs.

## Settings split (hybrid resolution)

Three layers resolved **field-by-field**:

```
effective(field) = tenant_override(field)  if non-NULL
                   else instance_default(field)      # system_settings

   org KB      → organization_settings[org]  → system_settings
   personal KB → user_settings[user]         → system_settings
```

- **Instance settings** (operator-only): the existing global `/settings` —
  default provider mode/base URL/key/model/embedding/timeout, search provider,
  language, default query limit.
- **Org settings page**: optional provider/key/model override + member management
  + per-KB grants.
- **Personal settings page**: optional provider/key/model override + profile.
- Everything works on the instance default until a tenant overrides a field.

The provider/settings *resolver* (currently reading the `system_settings`
singleton) is the integration point: it must take the owning space as input and
walk tenant → instance fallback. This is the main backend touch beyond the new
CRUD endpoints.

## Admin UI / navigation

**New**
- Signup page.
- Space switcher (top-level context selector: Personal + each org).
- Org settings page: members list + add/remove + role; per-KB grant management;
  org provider override.
- Personal settings page: personal provider override + profile.
- Per-KB "manage access" UI (grant viewer/editor to org members).
- "Move KB to an org" action on a personal KB.

**Changed**
- Projects list scoped to the active space.
- Global `/settings` and `/users` become **operator-only** (gated by
  `users.role = 'operator'`).

**Unchanged**
- Every per-project screen: files, sources, search, query, chat, graph, tasks,
  reviews, dedup, etc.

## Migration

One migration (next sequential number after `0011`):

1. Create `organizations`, `organization_members`, `spaces`,
   `organization_settings`, `user_settings`.
2. Add `projects.space_id` (nullable initially).
3. Backfill:
   - For **every** existing user, create a personal `spaces` row.
   - Set **all existing `projects.space_id`** to the seeded admin's personal
     space (decision: admin keeps all current KBs personally; can move to an org
     later).
   - `UPDATE project_members SET role='owner' WHERE role='project_owner';`
   - `UPDATE users SET role='operator' WHERE role='admin';`
     all other users → `role='user'`.
4. Enforce `projects.space_id NOT NULL` after backfill.

Backward-compat shims are explicitly avoided — this is pre-production and the
schema is changed directly.

## Out of scope

- Billing, plans, usage quotas, metering.
- Email-based invitations (members are added from existing registered users).
- Sharing a **personal** KB with other users (personal = private to its owner for
  now; org KBs are the sharing mechanism).
- SSO / external identity providers.

## Open implementation notes (for the plan, not new decisions)

- Decide ID strategy for new tables (match existing `projects.id` format).
- `slug` generation/validation rules for organizations.
- Where the resolver lives (extend the existing settings loader to accept a space
  context) and how `knowledge-core` receives resolved provider config (it is
  provider-agnostic; config is passed in, so no core change expected).
- Zod schemas in `packages/api-client/src/schemas.ts` for all new endpoints.
