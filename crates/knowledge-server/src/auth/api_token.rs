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

pub async fn list_tokens_for_user_scoped(
  state: &AppState,
  user_id: &str,
  scope: Option<&str>,
) -> Result<Vec<ApiTokenRecord>, ApiError> {
  let rows = match scope {
    None => {
      sqlx::query_as::<
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
    }
    Some(project_id) => {
      sqlx::query_as::<
        _,
        (String, String, Option<String>, String, String, Option<String>, Option<String>, String),
      >(
        "SELECT id, user_id, project_id, name, token_prefix, last_used_at, revoked_at, created_at
         FROM api_tokens
         WHERE user_id = $1 AND project_id = $2
         ORDER BY created_at DESC",
      )
      .bind(user_id)
      .bind(project_id)
      .fetch_all(&state.pool)
      .await
    }
  }
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

pub async fn list_tokens_for_user(
  state: &AppState,
  user_id: &str,
) -> Result<Vec<ApiTokenRecord>, ApiError> {
  list_tokens_for_user_scoped(state, user_id, None).await
}

pub async fn revoke_token_scoped(
  state: &AppState,
  user_id: &str,
  scope: Option<&str>,
  token_id: &str,
) -> Result<bool, ApiError> {
  let now = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format revoked_at"))?;
  let result = match scope {
    None => {
      sqlx::query(
        "UPDATE api_tokens
         SET revoked_at = $1
         WHERE id = $2 AND user_id = $3 AND revoked_at IS NULL",
      )
      .bind(&now)
      .bind(token_id)
      .bind(user_id)
      .execute(&state.pool)
      .await
    }
    Some(project_id) => {
      sqlx::query(
        "UPDATE api_tokens
         SET revoked_at = $1
         WHERE id = $2 AND user_id = $3 AND project_id = $4 AND revoked_at IS NULL",
      )
      .bind(&now)
      .bind(token_id)
      .bind(user_id)
      .bind(project_id)
      .execute(&state.pool)
      .await
    }
  }
  .map_err(ApiError::from)?;
  Ok(result.rows_affected() > 0)
}

pub async fn revoke_token(
  state: &AppState,
  user_id: &str,
  token_id: &str,
) -> Result<bool, ApiError> {
  revoke_token_scoped(state, user_id, None, token_id).await
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
