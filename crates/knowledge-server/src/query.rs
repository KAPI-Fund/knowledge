use crate::app::state::AppState;
use crate::http::error::ApiError;

#[derive(Debug)]
pub(crate) struct QuerySettings {
  pub(crate) language: String,
  pub(crate) default_query_limit: i64,
}

pub(crate) async fn load_query_settings(state: &AppState) -> Result<QuerySettings, ApiError> {
  let (language, default_query_limit) =
    sqlx::query_as::<_, (String, i64)>(
      "SELECT language, default_query_limit FROM system_settings WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

  Ok(QuerySettings { language, default_query_limit })
}
