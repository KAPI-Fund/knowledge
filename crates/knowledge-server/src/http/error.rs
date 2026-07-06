use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// Retry hint carried by an [`ApiError`] that originated from a background-task
/// executor. The HTTP layer ignores it; the task scheduler reads it to decide
/// whether a failure is transient (retry with backoff) or permanent. Without
/// this, a provider error's `retryable` flag was lost the moment an executor
/// returned `Result<_, ApiError>`, so every gateway blip killed the task on the
/// first attempt.
#[derive(Debug, Clone)]
pub struct RetryHint {
  pub code: String,
  pub retryable: bool,
  pub provider_status: Option<u16>,
}

#[derive(Debug)]
pub struct ApiError {
  status: StatusCode,
  message: String,
  retry: Option<RetryHint>,
}

impl ApiError {
  pub fn bad_request(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::BAD_REQUEST,
      message: message.into(),
      retry: None,
    }
  }

  pub fn forbidden(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::FORBIDDEN,
      message: message.into(),
      retry: None,
    }
  }

  pub fn internal(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::INTERNAL_SERVER_ERROR,
      message: message.into(),
      retry: None,
    }
  }

  pub fn not_found(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::NOT_FOUND,
      message: message.into(),
      retry: None,
    }
  }

  pub fn payload_too_large(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::PAYLOAD_TOO_LARGE,
      message: message.into(),
      retry: None,
    }
  }

  pub fn unsupported_media_type(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
      message: message.into(),
      retry: None,
    }
  }

  pub fn unauthorized(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::UNAUTHORIZED,
      message: message.into(),
      retry: None,
    }
  }

  /// Attach a task retry hint so the scheduler can preserve `retryable`/`code`
  /// across the executor's `Result<_, ApiError>` boundary.
  pub fn with_retry_hint(mut self, hint: RetryHint) -> Self {
    self.retry = Some(hint);
    self
  }

  /// The retry hint, if this error came from a task executor.
  pub fn retry_hint(&self) -> Option<&RetryHint> {
    self.retry.as_ref()
  }

  /// Map a SQLx error to 400 when it is a Postgres unique-constraint violation
  /// (SQLSTATE 23505); otherwise treat it as an internal error. Lets handlers
  /// rely on the DB's UNIQUE index instead of a racy COUNT precheck.
  pub fn from_db_unique(error: sqlx::Error, conflict_message: impl Into<String>) -> Self {
    if let sqlx::Error::Database(db_error) = &error {
      if db_error.code().as_deref() == Some("23505") {
        return Self::bad_request(conflict_message);
      }
    }
    Self::internal(error.to_string())
  }
}

impl From<sqlx::Error> for ApiError {
  fn from(value: sqlx::Error) -> Self {
    Self::internal(value.to_string())
  }
}

impl std::fmt::Display for ApiError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    #[derive(Serialize)]
    struct ErrorBody<'a> {
      error: &'a str,
    }

    (self.status, Json(ErrorBody { error: &self.message })).into_response()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn from_db_unique_maps_non_database_error_to_internal() {
    let err = ApiError::from_db_unique(sqlx::Error::RowNotFound, "slug already taken");
    assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
  }
}
