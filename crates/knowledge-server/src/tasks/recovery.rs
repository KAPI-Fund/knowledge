use crate::app::state::AppState;
use crate::http::error::ApiError;

pub async fn recover_tasks(state: &AppState) -> Result<(), ApiError> {
  sqlx::query(
    "UPDATE project_tasks
     SET status = 'queued',
         updated_at = NOW()::text,
         lease_owner = NULL,
         lease_expires_at = NULL
     WHERE status IN ('queued', 'running', 'retry_waiting')",
  )
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(())
}
