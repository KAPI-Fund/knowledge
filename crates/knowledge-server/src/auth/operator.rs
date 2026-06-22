use axum::http::HeaderMap;

use crate::app::state::AppState;
use crate::auth::principal::extract_session_cookie;
use crate::auth::session::{find_session, SessionRecord};
use crate::http::error::ApiError;

/// Resolve the caller's session and require `users.role = 'operator'`.
///
/// Instance-admin endpoints (`/api/system/settings`, `/api/users`) are
/// operator-only: an authenticated non-operator gets 403.
pub async fn require_operator(
  state: &AppState,
  headers: &HeaderMap,
) -> Result<SessionRecord, ApiError> {
  let session_id =
    extract_session_cookie(headers).ok_or_else(|| ApiError::unauthorized("missing session"))?;
  let session = find_session(state, &session_id)
    .await?
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;
  let role = sqlx::query_scalar::<_, String>("SELECT role FROM users WHERE id = $1")
    .bind(&session.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::from)?;
  if role.as_deref() != Some("operator") {
    return Err(ApiError::forbidden("operator role required"));
  }
  Ok(session)
}
