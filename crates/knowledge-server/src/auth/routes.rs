use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::auth::api_token::{
  create_api_token, list_tokens_for_user, revoke_token, CreateApiTokenInput,
};
use crate::auth::password::{hash_password, verify_password};
use crate::auth::principal::resolve_principal;
use crate::auth::session::{create_session, destroy_session, find_session};
use crate::http::error::ApiError;

pub fn router() -> Router<AppState> {
  Router::new()
    .route("/api/auth/login", post(login))
    .route("/api/auth/logout", post(logout))
    .route("/api/auth/me", get(me))
    .route(
      "/api/users/me/api-tokens",
      get(list_api_tokens_handler).post(create_api_token_handler),
    )
    .route(
      "/api/users/me/api-tokens/{token_id}/revoke",
      post(revoke_api_token_handler),
    )
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiTokenRequest {
  pub name: String,
  #[serde(default)]
  pub project_id: Option<String>,
}

async fn create_api_token_handler(
  State(state): State<AppState>,
  request: Request,
) -> Result<impl IntoResponse, ApiError> {
  let (parts, body) = request.into_parts();
  let principal = resolve_principal(&state, &parts.headers).await?;
  if principal.requires_csrf() {
    let expected = principal
      .csrf_token
      .as_deref()
      .ok_or_else(|| ApiError::unauthorized("missing csrf token"))?;
    let supplied = parts
      .headers
      .get("x-csrf-token")
      .and_then(|value| value.to_str().ok())
      .unwrap_or_default();
    if supplied.is_empty() || supplied != expected {
      return Err(ApiError::unauthorized("invalid csrf token"));
    }
  }

  let bytes = axum::body::to_bytes(body, 64 * 1024)
    .await
    .map_err(|_| ApiError::bad_request("invalid body"))?;
  let payload: CreateApiTokenRequest =
    serde_json::from_slice(&bytes).map_err(|error| ApiError::bad_request(error.to_string()))?;
  let trimmed = payload.name.trim();
  if trimmed.is_empty() || trimmed.len() > 100 {
    return Err(ApiError::bad_request("name must be 1\u{2013}100 characters"));
  }
  if let Some(project_id) = payload.project_id.as_deref() {
    let membership = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM project_members WHERE project_id = $1 AND user_id = $2",
    )
    .bind(project_id)
    .bind(&principal.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;
    if membership == 0 {
      return Err(ApiError::forbidden("not a project member"));
    }
  }

  let (record, token) = create_api_token(
    &state,
    CreateApiTokenInput {
      user_id: &principal.user_id,
      project_id: payload.project_id.as_deref(),
      name: trimmed,
    },
  )
  .await?;

  Ok((
    StatusCode::CREATED,
    Json(json!({
      "id": record.id,
      "name": record.name,
      "projectId": record.project_id,
      "prefix": record.token_prefix,
      "createdAt": record.created_at,
      "token": token
    })),
  ))
}

async fn list_api_tokens_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
  let principal = resolve_principal(&state, &headers).await?;
  let tokens = list_tokens_for_user(&state, &principal.user_id).await?;
  let payload = tokens
    .into_iter()
    .map(|token| {
      json!({
        "id": token.id,
        "name": token.name,
        "projectId": token.project_id,
        "prefix": token.token_prefix,
        "lastUsedAt": token.last_used_at,
        "revokedAt": token.revoked_at,
        "createdAt": token.created_at
      })
    })
    .collect::<Vec<_>>();
  Ok(Json(json!({ "tokens": payload })))
}

async fn revoke_api_token_handler(
  State(state): State<AppState>,
  Path(token_id): Path<String>,
  headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
  let principal = resolve_principal(&state, &headers).await?;
  if principal.requires_csrf() {
    let expected = principal
      .csrf_token
      .as_deref()
      .ok_or_else(|| ApiError::unauthorized("missing csrf token"))?;
    let supplied = headers
      .get("x-csrf-token")
      .and_then(|value| value.to_str().ok())
      .unwrap_or_default();
    if supplied.is_empty() || supplied != expected {
      return Err(ApiError::unauthorized("invalid csrf token"));
    }
  }
  let revoked = revoke_token(&state, &principal.user_id, &token_id).await?;
  if !revoked {
    return Err(ApiError::not_found("token not found"));
  }
  Ok(Json(json!({ "revoked": true })))
}
