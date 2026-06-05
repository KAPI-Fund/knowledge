use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;

#[derive(Debug, Clone)]
pub struct SessionRecord {
  pub id: String,
  pub user_id: String,
  pub csrf_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedSessionRecord {
  id: String,
  user_id: String,
  csrf_token: String,
  expires_at: String,
}

pub async fn create_session(state: &AppState, user_id: &str) -> Result<SessionRecord, ApiError> {
  let id = Uuid::new_v4().to_string();
  let csrf_token = Uuid::new_v4().to_string();
  let created_at = OffsetDateTime::now_utc();
  let expires_at = created_at + Duration::hours(state.session_ttl_hours as i64);
  let expires_at = expires_at
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format expiration"))?;
  let created_at = created_at
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format created_at"))?;

  sqlx::query(
    "INSERT INTO sessions (id, user_id, csrf_token, expires_at, created_at) VALUES ($1, $2, $3, $4, $5)",
  )
  .bind(&id)
  .bind(user_id)
  .bind(&csrf_token)
  .bind(&expires_at)
  .bind(&created_at)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  state
    .cache
    .set_json(
      &session_cache_key(&id),
      &CachedSessionRecord {
        id: id.clone(),
        user_id: user_id.to_string(),
        csrf_token: csrf_token.clone(),
        expires_at: expires_at.clone(),
      },
      ttl_seconds(&expires_at)?,
    )
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?;

  Ok(SessionRecord {
    id,
    user_id: user_id.to_string(),
    csrf_token,
  })
}

pub async fn find_session(
  state: &AppState,
  session_id: &str,
) -> Result<Option<SessionRecord>, ApiError> {
  if let Some(session) = state
    .cache
    .get_json::<CachedSessionRecord>(&session_cache_key(session_id))
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
  {
    if is_expired(&session.expires_at)? {
      state
        .cache
        .delete(&session_cache_key(session_id))
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
      return Ok(None);
    }

    return Ok(Some(SessionRecord {
      id: session.id,
      user_id: session.user_id,
      csrf_token: session.csrf_token,
    }));
  }

  let row = sqlx::query_as::<_, (String, String, String, String)>(
    "SELECT id, user_id, csrf_token, expires_at FROM sessions WHERE id = $1",
  )
  .bind(session_id)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?;

  let Some((id, user_id, csrf_token, expires_at)) = row else {
    return Ok(None);
  };

  if is_expired(&expires_at)? {
    destroy_session(state, session_id).await?;
    return Ok(None);
  }

  state
    .cache
    .set_json(
      &session_cache_key(session_id),
      &CachedSessionRecord {
        id: id.clone(),
        user_id: user_id.clone(),
        csrf_token: csrf_token.clone(),
        expires_at: expires_at.clone(),
      },
      ttl_seconds(&expires_at)?,
    )
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?;

  Ok(Some(SessionRecord {
    id,
    user_id,
    csrf_token,
  }))
}

pub async fn destroy_session(state: &AppState, session_id: &str) -> Result<(), ApiError> {
  sqlx::query("DELETE FROM sessions WHERE id = $1")
    .bind(session_id)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

  state
    .cache
    .delete(&session_cache_key(session_id))
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?;

  Ok(())
}

fn ttl_seconds(expires_at: &str) -> Result<u64, ApiError> {
  let expires_at = OffsetDateTime::parse(expires_at, &Rfc3339)
    .map_err(|_| ApiError::internal("failed to parse expiration"))?;
  Ok((expires_at - OffsetDateTime::now_utc()).whole_seconds().max(1) as u64)
}

fn is_expired(expires_at: &str) -> Result<bool, ApiError> {
  let expires_at = OffsetDateTime::parse(expires_at, &Rfc3339)
    .map_err(|_| ApiError::internal("failed to parse expiration"))?;
  Ok(expires_at <= OffsetDateTime::now_utc())
}

fn session_cache_key(session_id: &str) -> String {
  format!("session:{session_id}")
}
