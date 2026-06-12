use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::service::project_root_for_id;
use crate::providers::{OpenAiCompatibleProvider, ProviderError, ProviderQueryRequest};
use crate::retrieval::service::search_project_hybrid;
use knowledge_core::search::{SearchOptions, SearchResult};

#[derive(Debug, Clone)]
pub struct ExecuteProjectQueryInput {
  pub query: String,
  pub top_k: Option<usize>,
  pub language: Option<String>,
}

#[derive(Debug)]
pub struct QueryExecutionError {
  api_error: ApiError,
  code: String,
  retryable: bool,
  provider_status: Option<u16>,
}

impl QueryExecutionError {
  pub fn from_provider_error(error: ProviderError) -> Self {
    let api_error = if error.retryable() {
      ApiError::internal(error.message().to_string())
    } else {
      ApiError::bad_request(error.message().to_string())
    };

    Self {
      api_error,
      code: error.code().to_string(),
      retryable: error.retryable(),
      provider_status: error.provider_status(),
    }
  }

  pub fn retryable(&self) -> bool {
    self.retryable
  }

  pub fn code(&self) -> &str {
    &self.code
  }

  pub fn provider_status(&self) -> Option<u16> {
    self.provider_status
  }

  pub fn payload(&self, retryable: bool) -> Value {
    let mut payload = json!({
      "code": self.code,
      "message": self.api_error.to_string(),
      "retryable": retryable
    });

    if let Some(provider_status) = self.provider_status {
      payload["providerStatus"] = json!(provider_status);
    }

    payload
  }

  pub fn into_api_error(self) -> ApiError {
    self.api_error
  }
}

impl From<ApiError> for QueryExecutionError {
  fn from(api_error: ApiError) -> Self {
    Self {
      api_error,
      code: "task_execution_failed".to_string(),
      retryable: false,
      provider_status: None,
    }
  }
}

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

pub async fn execute_project_query(
  state: &AppState,
  project_id: &str,
  input: ExecuteProjectQueryInput,
) -> Result<Value, QueryExecutionError> {
  let settings = load_query_settings(state).await?;

  if settings.provider_mode.trim().is_empty()
    || settings
      .provider_base_url
      .as_deref()
      .unwrap_or("")
      .trim()
      .is_empty()
    || settings
      .provider_model
      .as_deref()
      .unwrap_or("")
      .trim()
      .is_empty()
  {
    return Err(ApiError::bad_request("provider configuration is incomplete").into());
  }

  if settings.provider_mode != "openai-compatible" {
    return Err(ApiError::bad_request("unsupported provider mode").into());
  }

  let top_k = input
    .top_k
    .unwrap_or(settings.default_query_limit.max(1) as usize)
    .max(1);
  let language = input
    .language
    .filter(|value| !value.trim().is_empty())
    .unwrap_or(settings.language);
  let root = project_root_for_id(state, project_id).await?;
  let results = search_project_hybrid(
    state,
    project_id,
    &root,
    &input.query,
    SearchOptions {
      top_k,
      include_content: false,
    },
  )
  .await
  .map_err(|error| ApiError::internal(error.to_string()))?;
  let selected_results = results.results;
  let context_blocks = build_context_blocks(&selected_results);
  let context_summary = build_context_summary(&selected_results);
  let provider = OpenAiCompatibleProvider::new(
    settings.provider_base_url.unwrap_or_default(),
    settings.provider_api_key.unwrap_or_default(),
    settings.provider_model.clone().unwrap_or_default(),
    settings.provider_timeout_seconds.unwrap_or(30),
  );
  let answer = provider
    .answer_query(ProviderQueryRequest {
      query: input.query,
      context_blocks,
      language,
    })
    .await
    .map_err(QueryExecutionError::from_provider_error)?;

  Ok(json!({
    "answer": answer.answer,
    "citations": selected_results
      .iter()
      .map(|result| {
        json!({
          "path": result.path,
          "title": result.title,
          "snippet": result.snippet,
          "score": result.score
        })
      })
      .collect::<Vec<_>>(),
    "contextSummary": context_summary,
    "model": settings.provider_model.unwrap_or_default(),
    "provider": settings.provider_mode,
    "usage": {
      "promptTokens": answer.usage.prompt_tokens,
      "completionTokens": answer.usage.completion_tokens,
      "totalTokens": answer.usage.total_tokens
    },
    "completedAt": now_rfc3339()?
  }))
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

fn build_context_blocks(results: &[SearchResult]) -> Vec<String> {
  if results.is_empty() {
    return vec!["No relevant wiki context was retrieved.".to_string()];
  }

  results
    .iter()
    .enumerate()
    .map(|(index, result)| {
      format!(
        "[{}] {}\nTitle: {}\nSnippet: {}",
        index + 1,
        result.path,
        result.title,
        result.snippet
      )
    })
    .collect()
}

fn build_context_summary(results: &[SearchResult]) -> String {
  results
    .iter()
    .map(|result| format!("{} ({})", result.path, result.title))
    .collect::<Vec<_>>()
    .join("; ")
}

fn now_rfc3339() -> Result<String, ApiError> {
  OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))
}
