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

pub async fn create_session(state: &AppState, user_id: &str) -> Result<SessionRecord, ApiError> {
  let id = Uuid::new_v4().to_string();
  let csrf_token = Uuid::new_v4().to_string();
  let created_at = OffsetDateTime::now_utc();
  let expires_at = created_at + Duration::hours(12);

  sqlx::query(
    "INSERT INTO sessions (id, user_id, csrf_token, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
  )
  .bind(&id)
  .bind(user_id)
  .bind(&csrf_token)
  .bind(expires_at.format(&Rfc3339).map_err(|_| ApiError::internal("failed to format expiration"))?)
  .bind(created_at.format(&Rfc3339).map_err(|_| ApiError::internal("failed to format created_at"))?)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

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
  let row = sqlx::query_as::<_, (String, String, String)>(
    "SELECT id, user_id, csrf_token FROM sessions WHERE id = ?1",
  )
  .bind(session_id)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(row.map(|(id, user_id, csrf_token)| SessionRecord {
    id,
    user_id,
    csrf_token,
  }))
}
