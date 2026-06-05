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
use crate::auth::session::{create_session, destroy_session, find_session};
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
    "SELECT id, username, password_hash FROM users WHERE username = $1",
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

async fn logout(
  State(state): State<AppState>,
  request: Request,
) -> Result<impl IntoResponse, ApiError> {
  if let Some(session_id) = request
    .headers()
    .get(header::COOKIE)
    .and_then(|value| value.to_str().ok())
    .and_then(|value| {
      value
        .split(';')
        .map(str::trim)
        .find(|item| item.starts_with("knowledge_session="))
        .map(|item| item.trim_start_matches("knowledge_session=").to_string())
    })
  {
    destroy_session(&state, &session_id).await?;
  }

  let mut response = StatusCode::OK.into_response();
  response.headers_mut().append(
    header::SET_COOKIE,
    HeaderValue::from_static("knowledge_session=; Max-Age=0; HttpOnly; Path=/; SameSite=Lax"),
  );
  Ok(response)
}

async fn me(
  State(state): State<AppState>,
  request: Request,
) -> Result<impl IntoResponse, ApiError> {
  let session_id = request
    .headers()
    .get(header::COOKIE)
    .and_then(|value| value.to_str().ok())
    .and_then(|value| {
      value
        .split(';')
        .map(str::trim)
        .find(|item| item.starts_with("knowledge_session="))
        .map(|item| item.trim_start_matches("knowledge_session=").to_string())
    })
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;

  let _session = find_session(&state, &session_id)
    .await?
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;

  let (id, username, role) = sqlx::query_as::<_, (String, String, String)>(
    "SELECT id, username, role FROM users WHERE id = $1",
  )
  .bind(&_session.user_id)
  .fetch_one(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(Json(serde_json::json!({
    "user": {
      "id": id,
      "username": username,
      "role": role
    }
  })))
}

async fn ensure_admin_user(
  state: &AppState,
  username: &str,
  password: &str,
) -> Result<(), ApiError> {
  let created_at = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format created_at"))?;

  sqlx::query(
    "INSERT INTO users (id, username, password_hash, role, created_at)
     VALUES ($1, $2, $3, $4, $5)
     ON CONFLICT (username) DO NOTHING",
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
