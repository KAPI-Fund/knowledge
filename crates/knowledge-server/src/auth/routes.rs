use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::auth::password::{hash_password, verify_password};
use crate::auth::session::create_session;
use crate::http::error::ApiError;

pub fn router() -> Router<AppState> {
  Router::new()
    .route("/api/auth/login", post(login))
    .route("/api/auth/logout", post(logout))
    .route("/api/auth/me", get(me))
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
  pub username: String,
  pub password: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
  pub csrf_token: String,
}

async fn login(
  State(state): State<AppState>,
  Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
  ensure_admin_user(&state, &payload.username, &payload.password).await?;

  let user = sqlx::query_as::<_, (String, String, String)>(
    "SELECT id, username, password_hash FROM users WHERE username = ?1",
  )
  .bind(&payload.username)
  .fetch_one(&state.pool)
  .await
  .map_err(ApiError::from)?;

  if !verify_password(&payload.password, &user.2)? {
    return Err(ApiError::unauthorized("invalid credentials"));
  }

  let session = create_session(&state, &user.0).await?;
  let mut response = Json(LoginResponse {
    csrf_token: session.csrf_token.clone(),
  })
  .into_response();

  response.headers_mut().append(
    header::SET_COOKIE,
    HeaderValue::from_str(&format!(
      "knowledge_session={}; HttpOnly; Path=/; SameSite=Lax",
      session.id
    ))
    .map_err(|_| ApiError::internal("failed to build session cookie"))?,
  );

  Ok(response)
}

async fn logout() -> StatusCode {
  StatusCode::OK
}

async fn me(request: Request) -> Result<StatusCode, ApiError> {
  let authorized = request
    .headers()
    .get(header::COOKIE)
    .and_then(|value| value.to_str().ok())
    .map(|value| value.contains("knowledge_session="))
    .unwrap_or(false);

  if authorized {
    Ok(StatusCode::OK)
  } else {
    Err(ApiError::unauthorized("missing session"))
  }
}

async fn ensure_admin_user(
  state: &AppState,
  username: &str,
  password: &str,
) -> Result<(), ApiError> {
  let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = ?1")
    .bind(username)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

  if existing > 0 {
    return Ok(());
  }

  let created_at = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format created_at"))?;

  sqlx::query(
    "INSERT INTO users (id, username, password_hash, role, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(username)
  .bind(hash_password(password)?)
  .bind("admin")
  .bind(created_at)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(())
}
