# Phase 4a — API Tokens Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add per-user, per-project-scoped API tokens that authenticate requests via `Authorization: Bearer <token>` as a parallel auth path alongside the existing session cookie. Tokens are minted from the admin UI, hashed at rest, listed/revoked from the same UI, and consumed by Phase 4b (MCP) and other external integrations.

**Architecture:** Three layers. (1) A new `api_tokens` table (id, user_id, project_id nullable, name, token_hash, token_prefix, last_used_at, revoked_at, created_at) plus a token generator + verifier (`crates/knowledge-server/src/auth/api_token.rs`) that ports upstream's `generateApiToken` (URL-safe base64, 32 bytes) and adds SHA-256 hashing. (2) An auth resolution layer (`crates/knowledge-server/src/auth/principal.rs`) joins the existing `find_session` with a new `find_by_token`; `authorized_session` in `projects/routes.rs` becomes `authorized_principal` and accepts either a session cookie or a Bearer token, returning a unified `Principal { user_id, csrf_token: Option<String>, scope: AuthScope, token_id: Option<String>, project_id: Option<String> }`. (3) New routes (`/api/users/me/api-tokens` create/list/revoke) and an admin UI feature module (`apps/admin/src/features/api-tokens/`) for managing tokens.

**Tech Stack:** Rust (axum 0.8, sqlx, sha2, base64, rand_core, time, uuid), React 19 + TanStack Query + Zod, vitest, Playwright.

**Upstream references (memory rule: port, don't invent):**
- `upstream_llm_wiki/src/lib/api-token.ts` — token generation primitive (32-byte URL-safe base64 generator), verbatim semantics.
- `upstream_llm_wiki/src/lib/api-token.test.ts` — entropy/uniqueness/URL-safety expectations; mirror as Rust tests.
- Upstream does NOT define DB storage, hashing, or per-user scoping (it uses a single env-var `LLM_WIKI_API_TOKEN`). This server is multi-tenant, so the storage layer, hashing, and per-user scoping are new code — but the generation primitive and its test invariants are ported verbatim.

**Documented divergences from upstream:**
1. Per-user (and optional per-project) scope vs upstream's single global `LLM_WIKI_API_TOKEN` env var. Reason: multi-tenant server.
2. SHA-256 hashing at rest (not Argon2, despite passwords using Argon2). Reason: token entropy is already 256 bits from a CSPRNG, so a fast hash is sufficient — Argon2 would only matter if tokens were low-entropy passwords. SHA-256 keeps lookup O(1) via an indexed hash column; Argon2 would force a row scan and a per-row verify.
3. CSRF: Bearer-token requests skip CSRF validation because there is no ambient credential to attack. Reason: same rationale as every API-token system in the industry — CSRF protects browser-mediated session cookies, not explicit Authorization headers.
4. Plaintext token is returned ONLY on creation (POST response body) and is never recoverable afterward. The DB only stores the hash and a fingerprint prefix (first 8 chars of the plaintext) for UI display.
5. Last-used timestamp is updated opportunistically: on successful auth, we issue a non-blocking UPDATE. Failures are logged but don't fail the request. Reason: avoid hot-path latency from a synchronous write.

**Indentation conventions:** `crates/**` source = 2-space, EXCEPT `crates/knowledge-server/src/projects/routes.rs` = 4-space and `crates/knowledge-server/src/auth/routes.rs` = 2-space. `tests/rust-integration/**` = 4-space. All TS/TSX = 2-space.

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `crates/knowledge-server/migrations/0009_api_tokens.sql` | Create | `api_tokens` table + indexes |
| `crates/knowledge-server/src/auth/api_token.rs` | Create | `generate_api_token`, `hash_token`, `fingerprint`, `create_api_token`, `find_by_token`, `list_tokens_for_user`, `revoke_token`, `touch_last_used` |
| `crates/knowledge-server/src/auth/mod.rs` | Modify | Register `api_token` module |
| `crates/knowledge-server/src/auth/routes.rs` | Modify | `/api/users/me/api-tokens` GET/POST + `/api/users/me/api-tokens/{id}/revoke` POST |
| `crates/knowledge-server/src/projects/routes.rs` | Modify | Rename `authorized_session` → `authorized_principal` + add Bearer resolution; `validate_csrf` becomes principal-aware (skip for Bearer) |
| `crates/knowledge-server/src/auth/principal.rs` | Create | `Principal` struct + `resolve_principal` (cookie OR bearer) |
| `tests/rust-integration/tests/api_tokens_api.rs` | Create | Bearer auth flow + scoping + revoke + last-used integration tests |
| `tests/rust-integration/tests/support/mod.rs` | Modify (none expected) | None (uses existing helpers) |
| `apps/admin/src/features/shared/api.ts` | Modify | 3 API functions + zod schemas |
| `apps/admin/src/features/api-tokens/queries.ts` | Create | Query + 2 mutations |
| `apps/admin/src/features/api-tokens/page.tsx` | Create | Token mint + list + revoke UI |
| `apps/admin/src/features/api-tokens/page.test.tsx` | Create | vitest component test |
| `apps/admin/src/app/router.tsx` | Modify | Route wiring under `/api-tokens` (system-level, not project-scoped) |
| `apps/admin/src/lib/route-meta.ts` | Modify | Nav tab |
| `tests/web/tests/api-tokens.spec.ts` | Create | e2e: mint a token, use it against an API endpoint, revoke it |

---

### Task 1: Database migration

**Files:**
- Create: `crates/knowledge-server/migrations/0009_api_tokens.sql`

The migrations directory uses sequential numbering (last existing = `0008_embedding_page_path_identity.sql`). SQLx runs migrations on startup via `sqlx::migrate!`.

- [ ] **Step 1: Create the migration file**

Create `crates/knowledge-server/migrations/0009_api_tokens.sql`:

```sql
CREATE TABLE api_tokens (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  project_id TEXT,
  name TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  token_prefix TEXT NOT NULL,
  last_used_at TEXT,
  revoked_at TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX api_tokens_user_id_idx ON api_tokens (user_id);
CREATE INDEX api_tokens_token_hash_idx ON api_tokens (token_hash);
```

Schema notes:
- `token_hash` is UNIQUE because SHA-256 collisions are vanishingly improbable for 256-bit input and the UNIQUE constraint defends against any future logic bug that tries to reuse a hash.
- `project_id` is nullable: null = system-wide token (still scoped to its owning user); non-null = restricted to one project.
- `token_prefix` is the first 8 chars of the plaintext token — used for UI display only ("k_T9aQ8x… (system, last used 2 days ago)") so revocation has a human-recognizable handle without leaking the secret.
- `revoked_at` null = active. We do NOT delete revoked rows: keeping them lets us return 401 instead of 404 for revoked tokens, which improves debuggability for users iterating on integrations.

- [ ] **Step 2: Verify the migration applies cleanly**

Postgres/Redis must be up (`docker compose up -d postgres redis` or `npm run docker:up`).

Run: `cargo build -p knowledge-server`
Then: `cargo run -p knowledge-server` for 2 seconds, then Ctrl+C.

Expected: server boots without error. The migration is applied on `bootstrap_state` startup. If it fails, the panic message will name the migration file.

- [ ] **Step 3: Commit**

```bash
git add crates/knowledge-server/migrations/0009_api_tokens.sql
git commit -m "feat: add api_tokens table migration"
```

---

### Task 2: Token generator + hasher (`auth/api_token.rs`)

**Files:**
- Create: `crates/knowledge-server/src/auth/api_token.rs`
- Modify: `crates/knowledge-server/src/auth/mod.rs`

Upstream reference: `upstream_llm_wiki/src/lib/api-token.ts` (`generateApiToken` — 32 bytes, base64url, no padding). The hasher + storage layer is new code.

- [ ] **Step 1: Register the module**

In `crates/knowledge-server/src/auth/mod.rs`:

```rust
pub mod api_token;
pub mod password;
pub mod routes;
pub mod session;
```

(Insert `api_token` alphabetically. The exact existing content of `mod.rs` should be 3 lines; adding `api_token` makes 4.)

- [ ] **Step 2: Write failing tests**

Create `crates/knowledge-server/src/auth/api_token.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn generate_api_token_is_url_safe_no_padding() {
    for _ in 0..32 {
      let token = generate_api_token();
      assert!(
        token.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'),
        "token contains non-url-safe character: {token}"
      );
      assert!(!token.contains('='));
      assert!(!token.contains('+'));
      assert!(!token.contains('/'));
    }
  }

  #[test]
  fn generate_api_token_has_at_least_43_chars() {
    // 32 random bytes -> 43 chars unpadded base64url.
    let token = generate_api_token();
    assert!(token.len() >= 43, "token too short: {} chars", token.len());
  }

  #[test]
  fn generate_api_token_is_unique_across_calls() {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for _ in 0..100 {
      assert!(seen.insert(generate_api_token()), "duplicate token");
    }
  }

  #[test]
  fn hash_token_is_deterministic_and_64_hex_chars() {
    let token = "fixed-test-token-value";
    let h1 = hash_token(token);
    let h2 = hash_token(token);
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64);
    assert!(h1.chars().all(|ch| ch.is_ascii_hexdigit()));
  }

  #[test]
  fn hash_token_differs_for_different_inputs() {
    assert_ne!(hash_token("a"), hash_token("b"));
  }

  #[test]
  fn fingerprint_is_first_8_chars_of_plaintext() {
    let token = "abcdefghijklmnop";
    assert_eq!(fingerprint(token), "abcdefgh");
  }

  #[test]
  fn fingerprint_handles_short_tokens() {
    assert_eq!(fingerprint("abc"), "abc");
  }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p knowledge-server auth::api_token`
Expected: FAIL — `cannot find function generate_api_token` (compile error).

- [ ] **Step 4: Implement the generator and hasher**

Prepend to `crates/knowledge-server/src/auth/api_token.rs`:

```rust
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;

/// Ported from upstream_llm_wiki/src/lib/api-token.ts generateApiToken.
/// 32 bytes from OsRng → URL-safe base64 no padding (43 chars).
pub fn generate_api_token() -> String {
  let mut bytes = [0u8; 32];
  OsRng.fill_bytes(&mut bytes);
  URL_SAFE_NO_PAD.encode(bytes)
}

/// SHA-256 hex digest. Used both for storage and for indexed lookup.
pub fn hash_token(token: &str) -> String {
  let mut hasher = Sha256::new();
  hasher.update(token.as_bytes());
  format!("{:x}", hasher.finalize())
}

/// First 8 chars of the plaintext (or the whole token, if shorter).
/// Stored verbatim in the DB for UI display — never used for auth.
pub fn fingerprint(token: &str) -> String {
  token.chars().take(8).collect()
}

#[derive(Debug, Clone)]
pub struct ApiTokenRecord {
  pub id: String,
  pub user_id: String,
  pub project_id: Option<String>,
  pub name: String,
  pub token_prefix: String,
  pub last_used_at: Option<String>,
  pub revoked_at: Option<String>,
  pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ApiTokenAuthRecord {
  pub id: String,
  pub user_id: String,
  pub project_id: Option<String>,
}

pub struct CreateApiTokenInput<'a> {
  pub user_id: &'a str,
  pub project_id: Option<&'a str>,
  pub name: &'a str,
}

/// Returns (record, plaintext_token). The plaintext is shown to the user
/// once and never persisted.
pub async fn create_api_token(
  state: &AppState,
  input: CreateApiTokenInput<'_>,
) -> Result<(ApiTokenRecord, String), ApiError> {
  let plaintext = generate_api_token();
  let id = Uuid::new_v4().to_string();
  let token_hash = hash_token(&plaintext);
  let token_prefix = fingerprint(&plaintext);
  let created_at = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format created_at"))?;

  sqlx::query(
    "INSERT INTO api_tokens (id, user_id, project_id, name, token_hash, token_prefix, created_at)
     VALUES ($1, $2, $3, $4, $5, $6, $7)",
  )
  .bind(&id)
  .bind(input.user_id)
  .bind(input.project_id)
  .bind(input.name)
  .bind(&token_hash)
  .bind(&token_prefix)
  .bind(&created_at)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok((
    ApiTokenRecord {
      id,
      user_id: input.user_id.to_string(),
      project_id: input.project_id.map(str::to_string),
      name: input.name.to_string(),
      token_prefix,
      last_used_at: None,
      revoked_at: None,
      created_at,
    },
    plaintext,
  ))
}

/// Hash-based lookup. Returns None for unknown OR revoked tokens.
pub async fn find_by_token(
  state: &AppState,
  plaintext: &str,
) -> Result<Option<ApiTokenAuthRecord>, ApiError> {
  let token_hash = hash_token(plaintext);
  let row = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
    "SELECT id, user_id, project_id, revoked_at FROM api_tokens WHERE token_hash = $1",
  )
  .bind(&token_hash)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?;

  let Some((id, user_id, project_id, revoked_at)) = row else {
    return Ok(None);
  };
  if revoked_at.is_some() {
    return Ok(None);
  }
  Ok(Some(ApiTokenAuthRecord {
    id,
    user_id,
    project_id,
  }))
}

pub async fn list_tokens_for_user(
  state: &AppState,
  user_id: &str,
) -> Result<Vec<ApiTokenRecord>, ApiError> {
  let rows = sqlx::query_as::<
    _,
    (String, String, Option<String>, String, String, Option<String>, Option<String>, String),
  >(
    "SELECT id, user_id, project_id, name, token_prefix, last_used_at, revoked_at, created_at
     FROM api_tokens
     WHERE user_id = $1
     ORDER BY created_at DESC",
  )
  .bind(user_id)
  .fetch_all(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(
    rows
      .into_iter()
      .map(|(id, user_id, project_id, name, token_prefix, last_used_at, revoked_at, created_at)| {
        ApiTokenRecord {
          id,
          user_id,
          project_id,
          name,
          token_prefix,
          last_used_at,
          revoked_at,
          created_at,
        }
      })
      .collect(),
  )
}

pub async fn revoke_token(
  state: &AppState,
  user_id: &str,
  token_id: &str,
) -> Result<bool, ApiError> {
  let now = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format revoked_at"))?;
  let result = sqlx::query(
    "UPDATE api_tokens SET revoked_at = $1 WHERE id = $2 AND user_id = $3 AND revoked_at IS NULL",
  )
  .bind(&now)
  .bind(token_id)
  .bind(user_id)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;
  Ok(result.rows_affected() > 0)
}

/// Non-fatal opportunistic write — caller logs errors and continues.
pub async fn touch_last_used(state: &AppState, token_id: &str) -> Result<(), ApiError> {
  let now = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format last_used_at"))?;
  sqlx::query("UPDATE api_tokens SET last_used_at = $1 WHERE id = $2")
    .bind(&now)
    .bind(token_id)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;
  Ok(())
}
```

(`AppState` exposes `pool: sqlx::PgPool` — see how `session.rs` uses it. `base64`, `rand_core`, `sha2`, `time`, `uuid` are all already workspace deps — see `crates/knowledge-server/Cargo.toml`.)

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p knowledge-server auth::api_token`
Expected: 7 unit tests PASS (no DB needed — the storage functions are exercised in Task 5's integration test).

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy -p knowledge-server --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/auth/api_token.rs crates/knowledge-server/src/auth/mod.rs
git commit -m "feat: add api token generator, hasher, and storage helpers"
```

---

### Task 3: Principal resolution layer (`auth/principal.rs`)

**Files:**
- Create: `crates/knowledge-server/src/auth/principal.rs`
- Modify: `crates/knowledge-server/src/auth/mod.rs`

This is the lynchpin change: today every protected route calls `authorized_session(state, headers, project_id)` which extracts the session cookie. We need to introduce a `Principal` that's either a session or an API token, and have the helper accept either credential. Doing the switch in one place — and renaming the function to `authorized_principal` — keeps the diff to call sites mechanical.

- [ ] **Step 1: Register the module**

In `crates/knowledge-server/src/auth/mod.rs`, after `api_token`:

```rust
pub mod principal;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/knowledge-server/src/auth/principal.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
  use super::*;
  use axum::http::HeaderMap;

  #[test]
  fn extract_bearer_token_reads_authorization_header() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer abc123".parse().unwrap());
    assert_eq!(extract_bearer_token(&headers), Some("abc123".to_string()));
  }

  #[test]
  fn extract_bearer_token_is_case_insensitive_on_scheme() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "bearer abc123".parse().unwrap());
    assert_eq!(extract_bearer_token(&headers), Some("abc123".to_string()));
  }

  #[test]
  fn extract_bearer_token_returns_none_when_missing() {
    let headers = HeaderMap::new();
    assert!(extract_bearer_token(&headers).is_none());
  }

  #[test]
  fn extract_bearer_token_returns_none_for_other_schemes() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Basic abc123".parse().unwrap());
    assert!(extract_bearer_token(&headers).is_none());
  }

  #[test]
  fn principal_requires_csrf_only_for_sessions() {
    let session = Principal {
      user_id: "user-1".to_string(),
      csrf_token: Some("csrf-1".to_string()),
      scope: AuthScope::Session,
      token_id: None,
      project_id: None,
    };
    let bearer = Principal {
      user_id: "user-1".to_string(),
      csrf_token: None,
      scope: AuthScope::ApiToken,
      token_id: Some("token-1".to_string()),
      project_id: None,
    };
    assert!(session.requires_csrf());
    assert!(!bearer.requires_csrf());
  }

  #[test]
  fn principal_project_scope_check_passes_when_token_is_unscoped() {
    let principal = Principal {
      user_id: "user-1".to_string(),
      csrf_token: None,
      scope: AuthScope::ApiToken,
      token_id: Some("token-1".to_string()),
      project_id: None,
    };
    assert!(principal.permits_project("any-project"));
  }

  #[test]
  fn principal_project_scope_check_passes_when_token_matches_project() {
    let principal = Principal {
      user_id: "user-1".to_string(),
      csrf_token: None,
      scope: AuthScope::ApiToken,
      token_id: Some("token-1".to_string()),
      project_id: Some("project-1".to_string()),
    };
    assert!(principal.permits_project("project-1"));
    assert!(!principal.permits_project("project-2"));
  }

  #[test]
  fn principal_project_scope_check_session_principals_always_permit() {
    let principal = Principal {
      user_id: "user-1".to_string(),
      csrf_token: Some("csrf-1".to_string()),
      scope: AuthScope::Session,
      token_id: None,
      project_id: None,
    };
    assert!(principal.permits_project("any-project"));
  }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p knowledge-server auth::principal`
Expected: FAIL — `cannot find type Principal` (compile error).

- [ ] **Step 4: Implement Principal + resolution**

Prepend to `crates/knowledge-server/src/auth/principal.rs`:

```rust
use axum::http::{HeaderMap, header};

use crate::app::state::AppState;
use crate::auth::api_token::{find_by_token, touch_last_used};
use crate::auth::session::find_session;
use crate::http::error::ApiError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthScope {
  Session,
  ApiToken,
}

#[derive(Debug, Clone)]
pub struct Principal {
  pub user_id: String,
  /// Present only for session principals.
  pub csrf_token: Option<String>,
  pub scope: AuthScope,
  /// Present only for api-token principals — used to update last_used_at.
  pub token_id: Option<String>,
  /// Present only for project-scoped api tokens. None = unscoped.
  pub project_id: Option<String>,
}

impl Principal {
  /// Sessions need CSRF validation on mutating routes; bearer tokens don't
  /// (no ambient credential to attack).
  pub fn requires_csrf(&self) -> bool {
    matches!(self.scope, AuthScope::Session)
  }

  /// True when this principal is allowed to act on the named project.
  pub fn permits_project(&self, project_id: &str) -> bool {
    match (&self.scope, &self.project_id) {
      (AuthScope::Session, _) => true,
      (AuthScope::ApiToken, None) => true,
      (AuthScope::ApiToken, Some(scope)) => scope == project_id,
    }
  }
}

pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
  let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
  let mut parts = value.splitn(2, char::is_whitespace);
  let scheme = parts.next()?;
  let token = parts.next()?;
  if !scheme.eq_ignore_ascii_case("bearer") {
    return None;
  }
  let trimmed = token.trim();
  if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

pub fn extract_session_cookie(headers: &HeaderMap) -> Option<String> {
  headers
    .get(header::COOKIE)
    .and_then(|value| value.to_str().ok())
    .and_then(|cookie| {
      cookie
        .split(';')
        .map(str::trim)
        .find(|item| item.starts_with("knowledge_session="))
        .map(|item| item.trim_start_matches("knowledge_session=").to_string())
    })
}

/// Resolve a Principal from cookie OR Bearer. Tries Bearer first because
/// API clients explicitly opt in by sending Authorization; if both are
/// present, the Bearer wins (the more explicit credential).
pub async fn resolve_principal(
  state: &AppState,
  headers: &HeaderMap,
) -> Result<Principal, ApiError> {
  if let Some(plaintext) = extract_bearer_token(headers)
    && let Some(token) = find_by_token(state, &plaintext).await?
  {
    if let Err(error) = touch_last_used(state, &token.id).await {
      tracing::warn!("failed to update api_tokens.last_used_at: {error}");
    }
    return Ok(Principal {
      user_id: token.user_id,
      csrf_token: None,
      scope: AuthScope::ApiToken,
      token_id: Some(token.id),
      project_id: token.project_id,
    });
  }

  let session_id =
    extract_session_cookie(headers).ok_or_else(|| ApiError::unauthorized("missing session"))?;
  let session = find_session(state, &session_id)
    .await?
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;
  Ok(Principal {
    user_id: session.user_id,
    csrf_token: Some(session.csrf_token),
    scope: AuthScope::Session,
    token_id: None,
    project_id: None,
  })
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p knowledge-server auth::principal`
Expected: 8 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/auth/principal.rs crates/knowledge-server/src/auth/mod.rs
git commit -m "feat: add Principal resolver for session and api-token auth"
```

---

### Task 4: Switch `authorized_session` to `authorized_principal`

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs` (4-space indent)

This is mechanical: replace one helper, leave its 30+ call sites unchanged (the helper signature is identical from the caller's perspective).

- [ ] **Step 1: Rename and rewrite the helper**

In `routes.rs`, replace the `authorized_session` function (currently at line ~1461) with:

```rust
pub(crate) async fn authorized_principal(
    state: &AppState,
    headers: &HeaderMap,
    project_id: Option<&str>,
) -> Result<crate::auth::principal::Principal, ApiError> {
    let principal = crate::auth::principal::resolve_principal(state, headers).await?;

    if let Some(project_id) = project_id {
        if !principal.permits_project(project_id) {
            return Err(ApiError::forbidden("api token is not scoped to this project"));
        }
        let membership = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM project_members WHERE project_id = $1 AND user_id = $2",
        )
        .bind(project_id)
        .bind(&principal.user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::from)?;
        if membership == 0 {
            return Err(ApiError::forbidden("not a project member"));
        }
    }
    Ok(principal)
}
```

Also delete the obsolete `extract_session_id` helper (line ~1365) — it's superseded by `extract_session_cookie` in `principal.rs`. Its only remaining caller is the function you just rewrote.

- [ ] **Step 2: Update `validate_csrf` to be principal-aware**

Replace the existing `validate_csrf` (line ~1378) with:

```rust
pub(crate) fn validate_csrf(
    headers: &HeaderMap,
    principal: &crate::auth::principal::Principal,
) -> Result<(), ApiError> {
    if !principal.requires_csrf() {
        return Ok(());
    }
    let expected = principal
        .csrf_token
        .as_deref()
        .ok_or_else(|| ApiError::unauthorized("missing csrf token"))?;
    let supplied = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if supplied.is_empty() || supplied != expected {
        return Err(ApiError::unauthorized("invalid csrf token"));
    }
    Ok(())
}
```

- [ ] **Step 3: Update every call site**

Run a single rename across `routes.rs`:

- `authorized_session(` → `authorized_principal(`
- `validate_csrf(&headers, &session.csrf_token)?;` → `validate_csrf(&headers, &session)?;`
- `let session = authorized_session(...)` → `let session = authorized_principal(...)` (variable name stays `session` — it's now a `Principal`, but renaming all the variables would balloon the diff. Live with the slight misnomer.)
- `session.user_id` → `session.user_id.clone()` where needed (still works — Principal exposes `user_id: String`)
- `session.csrf_token` references inside `validate_csrf` call sites — should already be passed via the new signature

Also: the same rename applies to other routers that use these helpers. Check:

```bash
grep -rn "authorized_session\|validate_csrf" crates/knowledge-server/src/
```

Update every match. The only files likely to be affected besides `projects/routes.rs` are any router-level handlers in `auth/`, `users/`, etc. — but those generally don't call `authorized_session` (auth routes manage their own auth).

- [ ] **Step 4: Make sure other routers compile**

Run: `cargo build -p knowledge-server`
Expected: clean compile. If unrelated routers had `extract_session_id` calls, replicate the helper there or import `auth::principal::extract_session_cookie`.

- [ ] **Step 5: Run the existing test suite to catch regressions**

Postgres/Redis must be up. Then:

Run: `cargo test -p rust-integration -- --test-threads=1`
Expected: all existing tests PASS (auth, search, dedup, etc.). Session-based auth must keep working exactly as before.

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/projects/routes.rs
git commit -m "refactor: replace authorized_session with principal-aware helper"
```

---

### Task 5: API token CRUD routes + integration test

**Files:**
- Modify: `crates/knowledge-server/src/auth/routes.rs` (2-space indent)
- Create: `tests/rust-integration/tests/api_tokens_api.rs` (4-space indent)

- [ ] **Step 1: Write the failing integration test**

Create `tests/rust-integration/tests/api_tokens_api.rs`. Harness helpers follow `search_api.rs` and `dedup_api.rs`:

```rust
mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn api_token_lifecycle_create_use_list_revoke() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("api-tokens-lifecycle").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("api-tokens-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

    // 1. Mint a token via session-authenticated POST.
    let mint = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "name": "test-token", "projectId": null }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mint.status(), StatusCode::CREATED);
    let body = read_json(mint.into_body()).await;
    let token = body["token"].as_str().unwrap().to_string();
    let token_id = body["id"].as_str().unwrap().to_string();
    assert!(body["prefix"].as_str().unwrap().len() >= 1);

    // 2. Use the token (Bearer) on a protected route — list projects.
    let listing = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/projects")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listing.status(), StatusCode::OK);

    // 3. Revoke the token.
    let revoke = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/users/me/api-tokens/{token_id}/revoke"))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoke.status(), StatusCode::OK);

    // 4. The revoked token must no longer authenticate.
    let after = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/projects")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(after.status(), StatusCode::UNAUTHORIZED);

    // 5. Listing tokens includes the revoked entry (UI history).
    let list = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/users/me/api-tokens")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let listed = read_json(list.into_body()).await;
    let tokens = listed["tokens"].as_array().unwrap();
    assert_eq!(tokens.len(), 1);
    assert!(tokens[0]["revokedAt"].as_str().is_some());

    let _ = project_id;
}

#[tokio::test]
async fn project_scoped_token_rejected_for_other_projects() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("api-tokens-scope").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_a_root = temp.path().join("project-a");
    let project_a = create_project(state.clone(), &cookie, &csrf, project_a_root).await;
    let project_b_root = temp.path().join("project-b");
    let project_b = create_project(state.clone(), &cookie, &csrf, project_b_root).await;

    // Mint a token scoped to project A.
    let mint = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "name": "scoped", "projectId": project_a }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mint.status(), StatusCode::CREATED);
    let token = read_json(mint.into_body())["token"].as_str().unwrap().to_string();

    // Project A should be accessible.
    let allowed = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_a}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);

    // Project B must be forbidden.
    let denied = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_b}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
}

async fn login_and_csrf(state: knowledge_server::app::state::AppState) -> (String, String) {
    let login = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                      "username": "admin",
                      "password": "secret-password"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let cookie = login
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let body = read_json(login.into_body()).await;
    let csrf = body
        .get("csrfToken")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    (cookie, csrf)
}

async fn create_project(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_root: std::path::PathBuf,
) -> String {
    support::create_project_with_alias(state, cookie, csrf, project_root).await
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p rust-integration --test api_tokens_api -- --test-threads=1`
Expected: FAIL — `/api/users/me/api-tokens` route returns 404.

- [ ] **Step 3: Add the routes and handlers**

In `crates/knowledge-server/src/auth/routes.rs`:

Add to the imports at the top (after the existing `use`s):

```rust
use crate::auth::api_token::{
  create_api_token, list_tokens_for_user, revoke_token, CreateApiTokenInput,
};
use crate::auth::principal::resolve_principal;
```

Extend `pub fn router()`:

```rust
pub fn router() -> Router<AppState> {
  Router::new()
    .route("/api/auth/login", post(login))
    .route("/api/auth/logout", post(logout))
    .route("/api/auth/me", get(me))
    .route(
      "/api/users/me/api-tokens",
      get(list_api_tokens_handler).post(create_api_token_handler),
    )
    .route(
      "/api/users/me/api-tokens/{token_id}/revoke",
      post(revoke_api_token_handler),
    )
}
```

Append the request struct and handlers (after the existing handlers):

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiTokenRequest {
  pub name: String,
  #[serde(default)]
  pub project_id: Option<String>,
}

async fn create_api_token_handler(
  State(state): State<AppState>,
  request: Request,
) -> Result<impl IntoResponse, ApiError> {
  let (parts, body) = request.into_parts();
  let principal = resolve_principal(&state, &parts.headers).await?;
  // CSRF only for session-authenticated mints — Bearer-authenticated mints
  // skip CSRF. (A Bearer token cannot mint another Bearer token if you also
  // require a session, but allowing Bearer→mint enables CLI automation.)
  if principal.requires_csrf() {
    let expected = principal
      .csrf_token
      .as_deref()
      .ok_or_else(|| ApiError::unauthorized("missing csrf token"))?;
    let supplied = parts
      .headers
      .get("x-csrf-token")
      .and_then(|value| value.to_str().ok())
      .unwrap_or_default();
    if supplied.is_empty() || supplied != expected {
      return Err(ApiError::unauthorized("invalid csrf token"));
    }
  }

  let bytes = axum::body::to_bytes(body, 64 * 1024)
    .await
    .map_err(|_| ApiError::bad_request("invalid body"))?;
  let payload: CreateApiTokenRequest =
    serde_json::from_slice(&bytes).map_err(|error| ApiError::bad_request(error.to_string()))?;
  let trimmed = payload.name.trim();
  if trimmed.is_empty() || trimmed.len() > 100 {
    return Err(ApiError::bad_request("name must be 1–100 characters"));
  }
  // If the user requests a project-scoped token, they must be a member.
  if let Some(project_id) = payload.project_id.as_deref() {
    let membership = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM project_members WHERE project_id = $1 AND user_id = $2",
    )
    .bind(project_id)
    .bind(&principal.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;
    if membership == 0 {
      return Err(ApiError::forbidden("not a project member"));
    }
  }

  let (record, token) = create_api_token(
    &state,
    CreateApiTokenInput {
      user_id: &principal.user_id,
      project_id: payload.project_id.as_deref(),
      name: trimmed,
    },
  )
  .await?;

  Ok((
    StatusCode::CREATED,
    Json(json!({
      "id": record.id,
      "name": record.name,
      "projectId": record.project_id,
      "prefix": record.token_prefix,
      "createdAt": record.created_at,
      "token": token
    })),
  ))
}

async fn list_api_tokens_handler(
  State(state): State<AppState>,
  headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
  let principal = resolve_principal(&state, &headers).await?;
  let tokens = list_tokens_for_user(&state, &principal.user_id).await?;
  let payload = tokens
    .into_iter()
    .map(|token| {
      json!({
        "id": token.id,
        "name": token.name,
        "projectId": token.project_id,
        "prefix": token.token_prefix,
        "lastUsedAt": token.last_used_at,
        "revokedAt": token.revoked_at,
        "createdAt": token.created_at
      })
    })
    .collect::<Vec<_>>();
  Ok(Json(json!({ "tokens": payload })))
}

async fn revoke_api_token_handler(
  State(state): State<AppState>,
  axum::extract::Path(token_id): axum::extract::Path<String>,
  headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
  let principal = resolve_principal(&state, &headers).await?;
  if principal.requires_csrf() {
    let expected = principal
      .csrf_token
      .as_deref()
      .ok_or_else(|| ApiError::unauthorized("missing csrf token"))?;
    let supplied = headers
      .get("x-csrf-token")
      .and_then(|value| value.to_str().ok())
      .unwrap_or_default();
    if supplied.is_empty() || supplied != expected {
      return Err(ApiError::unauthorized("invalid csrf token"));
    }
  }
  let revoked = revoke_token(&state, &principal.user_id, &token_id).await?;
  if !revoked {
    return Err(ApiError::not_found("token not found"));
  }
  Ok(Json(json!({ "revoked": true })))
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test api_tokens_api -- --test-threads=1`
Expected: both tests PASS.

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/auth/routes.rs tests/rust-integration/tests/api_tokens_api.rs
git commit -m "feat: add api token create, list, and revoke routes"
```

---

### Task 6: Admin API client + query hooks

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts`
- Create: `apps/admin/src/features/api-tokens/queries.ts`

Patterns: existing `updateProjectReview`/`sweepProjectReviews` (`api.ts` ~line 816) for mutations, `csrfHeader()` (~line 252), `apiFetch` + inline zod schemas.

- [ ] **Step 1: Add API functions**

In `apps/admin/src/features/shared/api.ts`, after the dedup section, add:

```ts
const apiTokenSchema = z.object({
  id: z.string(),
  name: z.string(),
  projectId: z.string().nullable(),
  prefix: z.string(),
  lastUsedAt: z.string().nullable(),
  revokedAt: z.string().nullable(),
  createdAt: z.string(),
});

const apiTokensListSchema = z.object({
  tokens: z.array(apiTokenSchema),
});

const apiTokenCreateResponseSchema = z.object({
  id: z.string(),
  name: z.string(),
  projectId: z.string().nullable(),
  prefix: z.string(),
  createdAt: z.string(),
  token: z.string(),
});

export type ApiTokenSummary = z.infer<typeof apiTokenSchema>;
export type ApiTokenCreateResponse = z.infer<typeof apiTokenCreateResponseSchema>;

export async function listApiTokens() {
  return apiFetch(`/api/users/me/api-tokens`, { method: "GET" }, apiTokensListSchema);
}

export async function createApiToken(input: { name: string; projectId: string | null }) {
  return apiFetch(
    `/api/users/me/api-tokens`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({ name: input.name, projectId: input.projectId }),
    },
    apiTokenCreateResponseSchema,
  );
}

export async function revokeApiToken(input: { tokenId: string }) {
  return apiFetch(
    `/api/users/me/api-tokens/${input.tokenId}/revoke`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    z.object({ revoked: z.boolean() }),
  );
}
```

- [ ] **Step 2: Create the query hooks**

Create `apps/admin/src/features/api-tokens/queries.ts`:

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { createApiToken, listApiTokens, revokeApiToken } from "../shared/api";

export function useApiTokensQuery() {
  return useQuery({
    queryKey: ["api-tokens"],
    queryFn: listApiTokens,
  });
}

function useApiTokensInvalidation() {
  const queryClient = useQueryClient();
  return async () => {
    await queryClient.invalidateQueries({ queryKey: ["api-tokens"] });
  };
}

export function useCreateApiTokenMutation() {
  const invalidate = useApiTokensInvalidation();
  return useMutation({
    mutationFn: createApiToken,
    onSuccess: async () => {
      await invalidate();
    },
  });
}

export function useRevokeApiTokenMutation() {
  const invalidate = useApiTokensInvalidation();
  return useMutation({
    mutationFn: revokeApiToken,
    onSuccess: async () => {
      await invalidate();
    },
  });
}
```

- [ ] **Step 3: Verify build**

Run: `npm run test --workspace @knowledge/admin`
Expected: existing tests PASS (no new tests yet — page test lands in Task 7).

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/features/shared/api.ts apps/admin/src/features/api-tokens/queries.ts
git commit -m "feat: add api token client functions and query hooks"
```

---

### Task 7: Admin API Tokens page + routing

**Files:**
- Create: `apps/admin/src/features/api-tokens/page.tsx`
- Create: `apps/admin/src/features/api-tokens/page.test.tsx`
- Modify: `apps/admin/src/app/router.tsx` (import + system-level route)
- Modify: `apps/admin/src/lib/route-meta.ts` (`systemRoutes` array, not `projectRoutes`)

Templates: `apps/admin/src/features/users/page.tsx` for the table layout, `apps/admin/src/features/dedup/page.test.tsx` for the vitest shape.

- [ ] **Step 1: Write the failing page test**

Create `apps/admin/src/features/api-tokens/page.test.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { ApiTokensPage } from "./page";

const mockTokensList = vi.fn();
const mockCreateToken = vi.fn();
const mockRevokeToken = vi.fn();

vi.mock("./queries", () => ({
  useApiTokensQuery: () => ({ data: mockTokensList() }),
  useCreateApiTokenMutation: () => ({ mutateAsync: mockCreateToken, isPending: false }),
  useRevokeApiTokenMutation: () => ({ mutateAsync: mockRevokeToken, isPending: false }),
}));

function renderPage() {
  const queryClient = new QueryClient();
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/api-tokens"]}>
        <ApiTokensPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("api tokens page", () => {
  it("mints a token and shows the plaintext once", async () => {
    const user = userEvent.setup();
    mockTokensList.mockReturnValue({ tokens: [] });
    mockCreateToken.mockResolvedValue({
      id: "token-1",
      name: "test",
      projectId: null,
      prefix: "abcd1234",
      createdAt: "2026-06-13T00:00:00Z",
      token: "plaintext-token-once",
    });

    renderPage();

    await user.type(screen.getByLabelText("Token Name"), "test");
    await user.click(screen.getByRole("button", { name: "Mint Token" }));

    expect(mockCreateToken).toHaveBeenCalledWith({ name: "test", projectId: null });
    expect(await screen.findByText("plaintext-token-once")).toBeInTheDocument();
    expect(
      screen.getByText(/Copy this token now — it will not be shown again/i),
    ).toBeInTheDocument();
  });

  it("lists existing tokens and revokes them", async () => {
    const user = userEvent.setup();
    mockTokensList.mockReturnValue({
      tokens: [
        {
          id: "token-1",
          name: "ci-bot",
          projectId: null,
          prefix: "abcd1234",
          lastUsedAt: "2026-06-13T00:00:00Z",
          revokedAt: null,
          createdAt: "2026-06-12T00:00:00Z",
        },
      ],
    });
    mockRevokeToken.mockResolvedValue({ revoked: true });

    renderPage();

    expect(screen.getByText("ci-bot")).toBeInTheDocument();
    expect(screen.getByText("abcd1234…")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Revoke" }));

    expect(mockRevokeToken).toHaveBeenCalledWith({ tokenId: "token-1" });
  });

  it("displays revoked tokens as revoked and hides their revoke button", () => {
    mockTokensList.mockReturnValue({
      tokens: [
        {
          id: "token-1",
          name: "old",
          projectId: null,
          prefix: "abcd1234",
          lastUsedAt: null,
          revokedAt: "2026-06-13T00:00:00Z",
          createdAt: "2026-06-12T00:00:00Z",
        },
      ],
    });

    renderPage();

    expect(screen.getByText("revoked")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Revoke" })).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- src/features/api-tokens/page.test.tsx`
Expected: FAIL — cannot resolve `./page`.

- [ ] **Step 3: Create the page component**

Create `apps/admin/src/features/api-tokens/page.tsx`:

```tsx
import { useState } from "react";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

import type { ApiTokenCreateResponse } from "../shared/api";

import {
  useApiTokensQuery,
  useCreateApiTokenMutation,
  useRevokeApiTokenMutation,
} from "./queries";

export function ApiTokensPage() {
  const [name, setName] = useState("");
  const [mintedToken, setMintedToken] = useState<ApiTokenCreateResponse | null>(null);
  const tokensQuery = useApiTokensQuery();
  const createMutation = useCreateApiTokenMutation();
  const revokeMutation = useRevokeApiTokenMutation();
  const tokens = tokensQuery.data?.tokens ?? [];

  return (
    <PageSection
      description="Mint and revoke API tokens for programmatic access. Tokens carry your user identity and project memberships."
      title="API Tokens"
    >
      <Card>
        <CardHeader>
          <CardTitle>Mint a New Token</CardTitle>
          <CardDescription>
            Provide a descriptive name. The token plaintext is shown once and is not recoverable
            afterwards.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-end">
          <label className="grid gap-2 text-sm font-medium">
            Token Name
            <Input
              aria-label="Token Name"
              onChange={(event) => setName(event.target.value)}
              placeholder="e.g. ci-bot"
              value={name}
            />
          </label>
          <Button
            disabled={createMutation.isPending || name.trim().length === 0}
            onClick={async () => {
              const result = await createMutation.mutateAsync({
                name: name.trim(),
                projectId: null,
              });
              setMintedToken(result);
              setName("");
            }}
          >
            Mint Token
          </Button>
        </CardContent>
      </Card>

      {mintedToken ? (
        <Card>
          <CardHeader>
            <CardTitle>New Token: {mintedToken.name}</CardTitle>
            <CardDescription>
              Copy this token now — it will not be shown again.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <code className="block break-all rounded bg-muted px-3 py-2 text-sm">
              {mintedToken.token}
            </code>
            <div className="mt-3 flex gap-2">
              <Button
                onClick={() => navigator.clipboard?.writeText(mintedToken.token).catch(() => undefined)}
                variant="outline"
              >
                Copy
              </Button>
              <Button onClick={() => setMintedToken(null)} variant="outline">
                Dismiss
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : null}

      {tokens.length ? (
        <Card>
          <CardHeader>
            <CardTitle>Existing Tokens</CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Prefix</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Last Used</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {tokens.map((token) => (
                  <TableRow key={token.id}>
                    <TableCell className="font-medium">{token.name}</TableCell>
                    <TableCell>
                      <code>{token.prefix}…</code>
                    </TableCell>
                    <TableCell>
                      {token.revokedAt ? (
                        <Badge variant="outline">revoked</Badge>
                      ) : (
                        <Badge>active</Badge>
                      )}
                    </TableCell>
                    <TableCell>{token.lastUsedAt ?? "—"}</TableCell>
                    <TableCell>{token.createdAt}</TableCell>
                    <TableCell className="text-right">
                      {token.revokedAt ? null : (
                        <Button
                          disabled={revokeMutation.isPending}
                          onClick={() => revokeMutation.mutateAsync({ tokenId: token.id })}
                          variant="outline"
                        >
                          Revoke
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      ) : (
        <EmptyState
          description="You haven't minted any API tokens yet."
          title="No tokens"
        />
      )}
    </PageSection>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- src/features/api-tokens/page.test.tsx`
Expected: PASS (3 tests).

- [ ] **Step 5: Wire routing**

In `apps/admin/src/app/router.tsx`, add the import alphabetically (after the existing `ApiTokens`-adjacent import — between `AuthGuard` and `AuditPage` is fine; check the actual file for the right slot):

```tsx
import { ApiTokensPage } from "../features/api-tokens/page";
```

API Tokens is system-wide, not project-scoped. Add the route INSIDE the `<Route path="/" element={<AppShell />}>` block, NEXT TO `users` and `settings` (NOT inside `projects/:projectId`):

```tsx
          <Route path="api-tokens" element={<ApiTokensPage />} />
```

In `apps/admin/src/lib/route-meta.ts`, add to `systemRoutes` after `"/users"`:

```ts
  { to: "/api-tokens", label: "API Tokens" },
```

- [ ] **Step 6: Run the full admin suite**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS, including the new api-tokens tests.

- [ ] **Step 7: Commit**

```bash
git add apps/admin/src/features/api-tokens/page.tsx apps/admin/src/features/api-tokens/page.test.tsx apps/admin/src/app/router.tsx apps/admin/src/lib/route-meta.ts
git commit -m "feat: add api tokens admin page with mint and revoke actions"
```

---

### Task 8: Playwright e2e — mint, use, revoke

**Files:**
- Create: `tests/web/tests/api-tokens.spec.ts`

The mock-openai server isn't relevant here — token endpoints don't hit the LLM. The spec walks the UI flow and then uses `page.request.get` with the minted token's `Authorization: Bearer` header.

- [ ] **Step 1: Write the e2e spec**

Create `tests/web/tests/api-tokens.spec.ts`:

```ts
import { expect, test } from "@playwright/test";

test("admin mints, uses, and revokes an api token", async ({ page }) => {
  await signInAsAdmin(page);

  await page.getByRole("link", { name: "API Tokens" }).click();
  await page.getByLabel("Token Name").fill("e2e-token");
  await page.getByRole("button", { name: "Mint Token" }).click();

  // The plaintext should appear inside the "New Token" card.
  const tokenCode = page.locator("code").first();
  await expect(tokenCode).toBeVisible();
  const token = (await tokenCode.textContent())?.trim() ?? "";
  expect(token.length).toBeGreaterThanOrEqual(43);

  // Use the token to hit a protected endpoint without a session cookie.
  const projects = await page.request.get("/api/projects", {
    headers: { authorization: `Bearer ${token}` },
  });
  expect(projects.ok()).toBeTruthy();

  // Dismiss the "new token" card so the existing-tokens table is the
  // only place the name appears.
  await page.getByRole("button", { name: "Dismiss" }).click();

  // Revoke from the existing-tokens table.
  await page.getByRole("button", { name: "Revoke" }).click();

  await expect(page.getByText("revoked").first()).toBeVisible({ timeout: 5_000 });

  // Re-use of the revoked token must 401.
  const denied = await page.request.get("/api/projects", {
    headers: { authorization: `Bearer ${token}` },
  });
  expect(denied.status()).toBe(401);
});

async function signInAsAdmin(page: import("@playwright/test").Page) {
  await page.goto("/login");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Password").fill("secret-password");
  await page.getByRole("button", { name: "Sign in" }).click();
  await page.waitForURL("**/projects");
}
```

- [ ] **Step 2: Run the spec**

Postgres/Redis must be up. Then:

Run: `npm run test --workspace @knowledge/web -- tests/api-tokens.spec.ts`
Expected: PASS.

- [ ] **Step 3: Run the full Playwright suite**

Run: `npm run test --workspace @knowledge/web`
Expected: PASS (7 existing + 1 new = 8 specs). The playwright config sets `workers: 1`, so settings races are not a concern.

- [ ] **Step 4: Commit**

```bash
git add tests/web/tests/api-tokens.spec.ts
git commit -m "test: add api-tokens e2e flow (mint, use, revoke)"
```

---

### Task 9: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Rust formatting and lint**

Run: `cargo fmt --all -- --check`
Expected: clean (no diffs). If anything is reported, run `cargo fmt --all` and commit.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 2: Rust tests**

Postgres/Redis up. Then:

Run: `cargo test --workspace -- --test-threads=1`
Expected: PASS, including `api_tokens_api` (2 integration tests) and the new unit tests in `auth::api_token` (7) and `auth::principal` (8).

- [ ] **Step 3: Frontend tests and lint**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS, including `src/features/api-tokens/page.test.tsx` (3 tests).

Run: `npm run lint`
Expected: tsc and clippy both clean.

- [ ] **Step 4: Playwright suite**

Run: `npm run test --workspace @knowledge/web`
Expected: PASS — all 8 specs including `api-tokens.spec.ts`.

- [ ] **Step 5: Final commit (only if fixes were needed)**

```bash
git status
```

If steps 1–4 required fixes, commit them. Otherwise the work is complete and there's nothing to commit.
