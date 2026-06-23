use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug)]
pub struct ApiError {
  status: StatusCode,
  message: String,
}

impl ApiError {
  pub fn bad_request(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::BAD_REQUEST,
      message: message.into(),
    }
  }

  pub fn forbidden(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::FORBIDDEN,
      message: message.into(),
    }
  }

  pub fn internal(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::INTERNAL_SERVER_ERROR,
      message: message.into(),
    }
  }

  pub fn not_found(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::NOT_FOUND,
      message: message.into(),
    }
  }

  pub fn payload_too_large(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::PAYLOAD_TOO_LARGE,
      message: message.into(),
    }
  }

  pub fn unsupported_media_type(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
      message: message.into(),
    }
  }

  pub fn unauthorized(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::UNAUTHORIZED,
      message: message.into(),
    }
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
