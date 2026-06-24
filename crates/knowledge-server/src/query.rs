use crate::app::state::AppState;
use crate::http::error::ApiError;

#[derive(Debug)]
pub(crate) struct QuerySettings {
  pub(crate) provider_mode: String,
  pub(crate) provider_base_url: Option<String>,
  pub(crate) provider_api_key: Option<String>,
  pub(crate) provider_model: Option<String>,
  pub(crate) provider_timeout_seconds: Option<i64>,
  pub(crate) language: String,
  pub(crate) default_query_limit: i64,
}

pub(crate) async fn load_query_settings(state: &AppState) -> Result<QuerySettings, ApiError> {
  let (
    provider_mode,
    provider_base_url,
    provider_api_key,
    provider_model,
    provider_timeout_seconds,
    language,
    default_query_limit,
  ) = sqlx::query_as::<_, (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    String,
    i64,
  )>(
    "SELECT
       provider_mode,
       provider_base_url,
       provider_api_key,
       provider_model,
       provider_timeout_seconds,
       language,
       default_query_limit
     FROM system_settings
     WHERE id = 1",
  )
  .fetch_one(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(QuerySettings {
    provider_mode,
    provider_base_url,
    provider_api_key,
    provider_model,
    provider_timeout_seconds,
    language,
    default_query_limit,
  })
}
