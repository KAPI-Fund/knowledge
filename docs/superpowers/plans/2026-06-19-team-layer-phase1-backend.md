# Team Layer — Plan 1: Backend Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the team layer to the backend data + authorization foundation: a new `teams`/`team_members` schema, `team` as a third `spaces.kind`, and an extended access resolver where org-space KBs are public (members default to `viewer`) and team-space KBs are private (leader + per-KB grants).

**Architecture:** A team becomes a first-class space kind. `migrations/0013_team_layer.sql` adds the tables and widens the `spaces` constraints. `tenancy::access::project_access_role` gains an `org` public-default branch and a new `team` branch. A new `tenancy::access::can_manage_kb_access` predicate backs the (later) grant-management endpoints. No HTTP endpoints or UI in this plan.

**Tech Stack:** Rust, Axum, SQLx (runtime string queries — no `query!` macro / no `.sqlx` metadata), Postgres 16. Migrations are embedded at compile time via `sqlx::migrate!("./migrations")` in `crates/knowledge-server/src/db/migrate.rs`, so adding a `.sql` file and rebuilding picks it up. Integration tests live in the `rust-integration` crate and use a real Postgres + Redis via `TestEnvironment` (requires Docker running).

**Spec:** `docs/superpowers/specs/2026-06-19-team-layer-design.md`

---

## File Structure

- **Create** `crates/knowledge-server/migrations/0013_team_layer.sql` — the `teams` + `team_members` tables and the `spaces` widening (add `team_id`, drop/recreate the two CHECKs, add the `spaces_team` unique index).
- **Modify** `crates/knowledge-server/src/tenancy/access.rs` — change the `org` branch default to `Viewer`, add the `team` branch to `project_access_role`, add the `can_manage_kb_access` predicate and a private `is_org_admin` helper.
- **Modify** `tests/rust-integration/tests/tenancy_access_api.rs` — add team test helpers (`insert_team`, `add_team_member`), a migration test, a team access-matrix test, a management-capability test, and flip the one Phase 1 expectation that the public-default semantic changes.

The existing test file uses **2-space indentation**; match it. `access.rs` uses **4-space indentation**; match it.

---

## Background the engineer needs

The current resolver (`crates/knowledge-server/src/tenancy/access.rs`) returns `Option<AccessRole>` where `AccessRole` is `Owner | Editor | Viewer`. Today the `org` branch returns `None` for an org member with no per-KB grant. **This plan changes that to `Viewer`** (org-space KBs are now "public" to all org members), and adds a `team` branch for the new private team KBs.

The `spaces` table was created in `migrations/0012_tenancy_foundation.sql` with two **inline, unnamed** CHECK constraints. Postgres auto-names a column-level `CHECK` on column `kind` as **`spaces_kind_check`** and the first table-level `CHECK` as **`spaces_check`**. Migration 0013 drops those two by name and recreates them. If a `DROP CONSTRAINT` ever fails with "constraint ... does not exist", run `\d spaces` against a DB migrated through 0012 to find the real names and update the two `DROP` lines — but the names above are the Postgres defaults and are expected to be correct.

Existing rows survive the new `spaces_owner_check`: personal rows have `owner_user_id` set with `org_id`/`team_id` NULL; org rows have `org_id` set with the others NULL (the new `team_id` column defaults to NULL).

---

### Task 1: Migration 0013 — teams, team_members, spaces widening

**Files:**
- Create: `crates/knowledge-server/migrations/0013_team_layer.sql`
- Test: `tests/rust-integration/tests/tenancy_access_api.rs` (append a new test)

- [ ] **Step 1: Write the failing test**

Append this test to the end of `tests/rust-integration/tests/tenancy_access_api.rs` (2-space indent):

```rust
#[tokio::test]
async fn team_migration_creates_tables_and_constraints() {
  let env = TestEnvironment::start("team-migration").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let pool = &state.pool;

  assert!(table_exists(pool, "teams").await.unwrap());
  assert!(table_exists(pool, "team_members").await.unwrap());

  // A team space (kind='team', team_id set) is accepted.
  let admin_id = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = 'admin'")
    .fetch_one(pool)
    .await
    .unwrap();
  let (org_id, _org_space) = insert_org(pool, &admin_id, "team-mig-org").await;
  let team_id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO teams (id, org_id, name, slug, created_by, created_at) \
     VALUES ($1, $2, 'Team A', 'team-a', $3, '2026-01-01T00:00:00Z')",
  )
  .bind(&team_id)
  .bind(&org_id)
  .bind(&admin_id)
  .execute(pool)
  .await
  .unwrap();

  let team_space = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO spaces (id, kind, owner_user_id, org_id, team_id, created_at) \
     VALUES ($1, 'team', NULL, NULL, $2, '2026-01-01T00:00:00Z')",
  )
  .bind(&team_space)
  .bind(&team_id)
  .execute(pool)
  .await
  .unwrap();

  // The partial unique index forbids a second space for the same team.
  let duplicate = sqlx::query(
    "INSERT INTO spaces (id, kind, owner_user_id, org_id, team_id, created_at) \
     VALUES ($1, 'team', NULL, NULL, $2, '2026-01-01T00:00:00Z')",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(&team_id)
  .execute(pool)
  .await;
  assert!(duplicate.is_err(), "second space for a team must be rejected");

  // The owner-check rejects a malformed team space (team_id + org_id both set).
  let malformed = sqlx::query(
    "INSERT INTO spaces (id, kind, owner_user_id, org_id, team_id, created_at) \
     VALUES ($1, 'team', NULL, $2, $3, '2026-01-01T00:00:00Z')",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(&org_id)
  .bind(&team_id)
  .execute(pool)
  .await;
  assert!(malformed.is_err(), "team space with org_id set must be rejected");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p rust-integration --test tenancy_access_api team_migration_creates_tables_and_constraints`
Expected: FAIL — `assertion failed: table_exists(pool, "teams")...` (the `teams` table does not exist yet because migration 0013 has not been added).

- [ ] **Step 3: Create the migration**

Create `crates/knowledge-server/migrations/0013_team_layer.sql`:

```sql
CREATE TABLE teams (
  id TEXT PRIMARY KEY NOT NULL,
  org_id TEXT NOT NULL,
  name TEXT NOT NULL,
  slug TEXT NOT NULL,
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(org_id, slug),
  FOREIGN KEY(org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE team_members (
  id TEXT PRIMARY KEY NOT NULL,
  team_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('leader', 'member')),
  created_at TEXT NOT NULL,
  UNIQUE(team_id, user_id),
  FOREIGN KEY(team_id) REFERENCES teams(id) ON DELETE CASCADE,
  FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

ALTER TABLE spaces ADD COLUMN team_id TEXT REFERENCES teams(id) ON DELETE CASCADE;

ALTER TABLE spaces DROP CONSTRAINT spaces_kind_check;
ALTER TABLE spaces DROP CONSTRAINT spaces_check;

ALTER TABLE spaces ADD CONSTRAINT spaces_kind_check
  CHECK (kind IN ('personal', 'org', 'team'));

ALTER TABLE spaces ADD CONSTRAINT spaces_owner_check CHECK (
  (kind = 'personal' AND owner_user_id IS NOT NULL AND org_id IS NULL AND team_id IS NULL) OR
  (kind = 'org' AND org_id IS NOT NULL AND owner_user_id IS NULL AND team_id IS NULL) OR
  (kind = 'team' AND team_id IS NOT NULL AND owner_user_id IS NULL AND org_id IS NULL)
);

CREATE UNIQUE INDEX spaces_team ON spaces(team_id) WHERE kind = 'team';
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test tenancy_access_api team_migration_creates_tables_and_constraints`
Expected: PASS. (Cargo rebuilds `knowledge-server`, re-embedding migrations including 0013.)

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/migrations/0013_team_layer.sql tests/rust-integration/tests/tenancy_access_api.rs
git commit -m "feat(tenancy): add team layer schema (teams, team_members, team space kind)"
```

---

### Task 2: Resolver — org-space KBs default to Viewer

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/access.rs` (the `"org"` match arm of `project_access_role`)
- Test: `tests/rust-integration/tests/tenancy_access_api.rs` (update `access_role_matrix`)

- [ ] **Step 1: Update the existing test to the new semantics**

In `tests/rust-integration/tests/tenancy_access_api.rs`, find the `ungranted` assertion inside `access_role_matrix` (currently expecting `None`) and change it to expect `Viewer`. Replace:

```rust
  assert_eq!(
    project_access_role(pool, &org_project, &ungranted).await.unwrap(),
    None
  );
```

with:

```rust
  // Org-space KBs are public: an org member with no grant reads as viewer.
  assert_eq!(
    project_access_role(pool, &org_project, &ungranted).await.unwrap(),
    Some(AccessRole::Viewer)
  );
```

Leave the `stranger` assertion (a non-member) as `None` — non-members still have no access.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p rust-integration --test tenancy_access_api access_role_matrix`
Expected: FAIL — the resolver still returns `None` for `ungranted`, so the assertion `Some(AccessRole::Viewer)` fails with `left: None, right: Some(Viewer)`.

- [ ] **Step 3: Change the org branch default**

In `crates/knowledge-server/src/tenancy/access.rs`, inside `project_access_role`, the `"org"` arm currently ends its grant `match` with `_ => Ok(None)`. Change only that fallback to `Viewer`. Replace this block:

```rust
                    match grant.as_deref() {
                        Some("owner") | Some("editor") => Ok(Some(AccessRole::Editor)),
                        Some("viewer") => Ok(Some(AccessRole::Viewer)),
                        _ => Ok(None),
                    }
```

with:

```rust
                    match grant.as_deref() {
                        Some("owner") | Some("editor") => Ok(Some(AccessRole::Editor)),
                        // Public org KB: any org member reads by default.
                        _ => Ok(Some(AccessRole::Viewer)),
                    }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test tenancy_access_api access_role_matrix`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tenancy/access.rs tests/rust-integration/tests/tenancy_access_api.rs
git commit -m "feat(tenancy): org-space KBs are public (members default to viewer)"
```

---

### Task 3: Resolver — team branch

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/access.rs` (`project_access_role` + a private `is_org_admin` helper)
- Test: `tests/rust-integration/tests/tenancy_access_api.rs` (new helpers + `team_access_role_matrix`)

- [ ] **Step 1: Add team test helpers**

In `tests/rust-integration/tests/tenancy_access_api.rs`, add these two helpers next to the existing `add_org_member` / `grant_kb` helpers (2-space indent):

```rust
async fn insert_team(pool: &sqlx::PgPool, org_id: &str, slug: &str) -> (String, String) {
  let team_id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO teams (id, org_id, name, slug, created_by, created_at) \
     SELECT $1, $2, $3, $3, om.user_id, '2026-01-01T00:00:00Z' \
     FROM organization_members om WHERE om.org_id = $2 AND om.role = 'org_admin' LIMIT 1",
  )
  .bind(&team_id)
  .bind(org_id)
  .bind(slug)
  .execute(pool)
  .await
  .unwrap();
  let space_id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO spaces (id, kind, owner_user_id, org_id, team_id, created_at) \
     VALUES ($1, 'team', NULL, NULL, $2, '2026-01-01T00:00:00Z')",
  )
  .bind(&space_id)
  .bind(&team_id)
  .execute(pool)
  .await
  .unwrap();
  (team_id, space_id)
}

async fn add_team_member(pool: &sqlx::PgPool, team_id: &str, user_id: &str, role: &str) {
  sqlx::query(
    "INSERT INTO team_members (id, team_id, user_id, role, created_at) \
     VALUES ($1, $2, $3, $4, '2026-01-01T00:00:00Z')",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(team_id)
  .bind(user_id)
  .bind(role)
  .execute(pool)
  .await
  .unwrap();
}
```

> `insert_team` sets `created_by` to the org's admin so the FK to `users` is satisfied; it requires the org to already have an `org_admin` member (the test below adds one first).

- [ ] **Step 2: Write the failing team matrix test**

Append to `tests/rust-integration/tests/tenancy_access_api.rs`:

```rust
#[tokio::test]
async fn team_access_role_matrix() {
  let env = TestEnvironment::start("team-access-matrix").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let pool = &state.pool;

  let admin = insert_user(pool, "tam-admin").await;
  let leader = insert_user(pool, "tam-leader").await;
  let granted_editor = insert_user(pool, "tam-editor").await;
  let granted_viewer = insert_user(pool, "tam-viewer").await;
  let no_grant = insert_user(pool, "tam-nogrant").await;
  let other_member = insert_user(pool, "tam-other").await;
  let stranger = insert_user(pool, "tam-stranger").await;

  let (org_id, _org_space) = insert_org(pool, &admin, "tam-org").await;
  add_org_member(pool, &org_id, &admin, "org_admin").await;
  add_org_member(pool, &org_id, &leader, "org_member").await;
  add_org_member(pool, &org_id, &granted_editor, "org_member").await;
  add_org_member(pool, &org_id, &granted_viewer, "org_member").await;
  add_org_member(pool, &org_id, &no_grant, "org_member").await;
  add_org_member(pool, &org_id, &other_member, "org_member").await;

  let (team_id, team_space) = insert_team(pool, &org_id, "team-a").await;
  let team_project = insert_project_in_space(pool, &team_space, "team-kb").await;

  add_team_member(pool, &team_id, &leader, "leader").await;
  add_team_member(pool, &team_id, &granted_editor, "member").await;
  add_team_member(pool, &team_id, &granted_viewer, "member").await;
  add_team_member(pool, &team_id, &no_grant, "member").await;
  grant_kb(pool, &team_project, &granted_editor, "editor").await;
  grant_kb(pool, &team_project, &granted_viewer, "viewer").await;

  // org_admin owns every org KB, including private team KBs.
  assert_eq!(
    project_access_role(pool, &team_project, &admin).await.unwrap(),
    Some(AccessRole::Owner)
  );
  // The team leader is editor by default.
  assert_eq!(
    project_access_role(pool, &team_project, &leader).await.unwrap(),
    Some(AccessRole::Editor)
  );
  // Granted members get exactly their grant.
  assert_eq!(
    project_access_role(pool, &team_project, &granted_editor).await.unwrap(),
    Some(AccessRole::Editor)
  );
  assert_eq!(
    project_access_role(pool, &team_project, &granted_viewer).await.unwrap(),
    Some(AccessRole::Viewer)
  );
  // A team member with no grant has no access.
  assert_eq!(
    project_access_role(pool, &team_project, &no_grant).await.unwrap(),
    None
  );
  // An org member who is not on the team has no access to a private team KB.
  assert_eq!(
    project_access_role(pool, &team_project, &other_member).await.unwrap(),
    None
  );
  // A non-member of the org has no access.
  assert_eq!(
    project_access_role(pool, &team_project, &stranger).await.unwrap(),
    None
  );
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p rust-integration --test tenancy_access_api team_access_role_matrix`
Expected: FAIL — the resolver has no `"team"` arm, so it hits `_ => Ok(None)` and every non-`None` assertion fails (the first failing assert is `admin` expected `Some(Owner)`, got `None`).

- [ ] **Step 4: Add the team branch and the `is_org_admin` helper**

In `crates/knowledge-server/src/tenancy/access.rs`:

First, extend the `space` query to also select `team_id`. Replace:

```rust
    let space = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT s.kind, s.owner_user_id, s.org_id \
         FROM spaces s JOIN projects p ON p.space_id = s.id \
         WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    let Some((kind, owner_user_id, org_id)) = space else {
        return Ok(None);
    };
```

with:

```rust
    let space = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>)>(
        "SELECT s.kind, s.owner_user_id, s.org_id, s.team_id \
         FROM spaces s JOIN projects p ON p.space_id = s.id \
         WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    let Some((kind, owner_user_id, org_id, team_id)) = space else {
        return Ok(None);
    };
```

Then add a new `"team"` arm to the `match kind.as_str()` block, immediately before the final `_ => Ok(None),`:

```rust
        "team" => {
            let Some(team_id) = team_id else {
                return Ok(None);
            };
            let owning_org = sqlx::query_scalar::<_, String>(
                "SELECT org_id FROM teams WHERE id = $1",
            )
            .bind(&team_id)
            .fetch_optional(pool)
            .await?;
            let Some(owning_org) = owning_org else {
                return Ok(None);
            };
            if is_org_admin(pool, &owning_org, user_id).await? {
                return Ok(Some(AccessRole::Owner));
            }
            let team_role = sqlx::query_scalar::<_, String>(
                "SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2",
            )
            .bind(&team_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            if team_role.as_deref() == Some("leader") {
                return Ok(Some(AccessRole::Editor));
            }
            let grant = sqlx::query_scalar::<_, String>(
                "SELECT role FROM project_members WHERE project_id = $1 AND user_id = $2",
            )
            .bind(project_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            match grant.as_deref() {
                Some("owner") | Some("editor") => Ok(Some(AccessRole::Editor)),
                Some("viewer") => Ok(Some(AccessRole::Viewer)),
                _ => Ok(None),
            }
        }
```

Finally, add this private helper at the end of the file (after `project_access_role`):

```rust
async fn is_org_admin(pool: &PgPool, org_id: &str, user_id: &str) -> Result<bool, sqlx::Error> {
    let role = sqlx::query_scalar::<_, String>(
        "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
    )
    .bind(org_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(role.as_deref() == Some("org_admin"))
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test tenancy_access_api team_access_role_matrix`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/tenancy/access.rs tests/rust-integration/tests/tenancy_access_api.rs
git commit -m "feat(tenancy): resolve access for private team-space KBs"
```

---

### Task 4: `can_manage_kb_access` predicate

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/access.rs` (add `can_manage_kb_access`)
- Test: `tests/rust-integration/tests/tenancy_access_api.rs` (new `can_manage_kb_access_matrix`)

- [ ] **Step 1: Extend the access import**

In `tests/rust-integration/tests/tenancy_access_api.rs`, find the mid-file import:

```rust
use knowledge_server::tenancy::access::{AccessRole, project_access_role};
```

and change it to:

```rust
use knowledge_server::tenancy::access::{AccessRole, can_manage_kb_access, project_access_role};
```

- [ ] **Step 2: Write the failing capability test**

Append to `tests/rust-integration/tests/tenancy_access_api.rs`:

```rust
#[tokio::test]
async fn can_manage_kb_access_matrix() {
  let env = TestEnvironment::start("team-manage-cap").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let pool = &state.pool;

  let admin = insert_user(pool, "cap-admin").await;
  let leader = insert_user(pool, "cap-leader").await;
  let other_leader = insert_user(pool, "cap-other-leader").await;
  let member = insert_user(pool, "cap-member").await;

  let (org_id, org_space) = insert_org(pool, &admin, "cap-org").await;
  add_org_member(pool, &org_id, &admin, "org_admin").await;
  add_org_member(pool, &org_id, &leader, "org_member").await;
  add_org_member(pool, &org_id, &other_leader, "org_member").await;
  add_org_member(pool, &org_id, &member, "org_member").await;

  let public_project = insert_project_in_space(pool, &org_space, "cap-public-kb").await;

  let (team_id, team_space) = insert_team(pool, &org_id, "cap-team").await;
  let team_project = insert_project_in_space(pool, &team_space, "cap-team-kb").await;
  add_team_member(pool, &team_id, &leader, "leader").await;
  add_team_member(pool, &team_id, &member, "member").await;
  grant_kb(pool, &team_project, &member, "editor").await;

  let (_other_team, other_team_space) = insert_team(pool, &org_id, "cap-other-team").await;
  let other_team_project = insert_project_in_space(pool, &other_team_space, "cap-other-kb").await;

  // org_admin can manage grants on every org KB.
  assert!(can_manage_kb_access(pool, &public_project, &admin).await.unwrap());
  assert!(can_manage_kb_access(pool, &team_project, &admin).await.unwrap());
  // A plain org member cannot manage a public KB's grants.
  assert!(!can_manage_kb_access(pool, &public_project, &member).await.unwrap());
  // The team leader manages their own team's KB.
  assert!(can_manage_kb_access(pool, &team_project, &leader).await.unwrap());
  // A granted (non-leader) member cannot manage grants.
  assert!(!can_manage_kb_access(pool, &team_project, &member).await.unwrap());
  // A leader of a different team cannot manage this team's KB.
  add_team_member(pool, &_other_team, &other_leader, "leader").await;
  assert!(!can_manage_kb_access(pool, &team_project, &other_leader).await.unwrap());
  assert!(can_manage_kb_access(pool, &other_team_project, &other_leader).await.unwrap());
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p rust-integration --test tenancy_access_api can_manage_kb_access_matrix`
Expected: FAIL to compile — `can_manage_kb_access` does not exist (error `E0432: unresolved import` / `cannot find function`).

- [ ] **Step 4: Implement `can_manage_kb_access`**

In `crates/knowledge-server/src/tenancy/access.rs`, add this public function (after `project_access_role`, reusing the `is_org_admin` helper from Task 3):

```rust
/// Whether `user_id` may manage per-KB access grants on `project_id`.
///
/// True for the owning org's `org_admin` (public or team KB), and for the
/// `leader` of the team that owns a team-space KB. Personal-space KBs have no
/// grant management (the owner controls them implicitly).
pub async fn can_manage_kb_access(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
) -> Result<bool, sqlx::Error> {
    let space = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT s.kind, s.org_id, s.team_id \
         FROM spaces s JOIN projects p ON p.space_id = s.id \
         WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    let Some((kind, org_id, team_id)) = space else {
        return Ok(false);
    };

    match kind.as_str() {
        "org" => {
            let Some(org_id) = org_id else {
                return Ok(false);
            };
            is_org_admin(pool, &org_id, user_id).await
        }
        "team" => {
            let Some(team_id) = team_id else {
                return Ok(false);
            };
            let owning_org = sqlx::query_scalar::<_, String>(
                "SELECT org_id FROM teams WHERE id = $1",
            )
            .bind(&team_id)
            .fetch_optional(pool)
            .await?;
            if let Some(owning_org) = owning_org {
                if is_org_admin(pool, &owning_org, user_id).await? {
                    return Ok(true);
                }
            }
            let team_role = sqlx::query_scalar::<_, String>(
                "SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2",
            )
            .bind(&team_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            Ok(team_role.as_deref() == Some("leader"))
        }
        _ => Ok(false),
    }
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test tenancy_access_api can_manage_kb_access_matrix`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/tenancy/access.rs tests/rust-integration/tests/tenancy_access_api.rs
git commit -m "feat(tenancy): add can_manage_kb_access predicate for grant management"
```

---

### Task 5: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Run the whole tenancy suite**

Run: `cargo test -p rust-integration --test tenancy_access_api`
Expected: PASS — all tests (the 5 Phase 1 tests plus the 3 new ones) green.

- [ ] **Step 2: Run the full integration suite to catch semantic-change fallout**

Run: `cargo test -p rust-integration`
Expected: PASS. The org-public-default change (Task 2) means any other test that placed a KB in an **org** space and expected an org member **without a grant** to be denied will now see `Viewer`. If such a test fails, update its expectation to match the new public-default semantics (org member with no grant → has `viewer` access). Do not weaken privacy tests for **personal**-space KBs or **non-members** — those still resolve to `None`.

- [ ] **Step 3: Run backend unit tests + clippy**

Run: `cargo test -p knowledge-server`
Expected: PASS.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean (no warnings).

- [ ] **Step 4: Commit any fixups**

If Step 2 required test-expectation updates, commit them:

```bash
git add -A
git commit -m "test(tenancy): align org-space access expectations with public-default semantics"
```

If nothing changed, skip this step.

---

## Notes for the executor

- This plan adds **no HTTP endpoints and no UI** — those are Plan 2 (backend endpoints) and Plan 3 (frontend) per the spec. `create_project` continues to use the caller's personal space; team/org KB creation arrives with the endpoints in Plan 2. Team KBs in this plan are exercised by inserting rows directly in the integration tests, exactly as Phase 1 exercised org KBs.
- SQLx here uses **runtime string queries**, not the `query!` macro, so there is no `.sqlx` offline metadata to regenerate.
- Integration tests require **Docker** (Postgres + Redis via `TestEnvironment`).
