use axum::extract::{Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::app::state::AppState;
use crate::auth::api_token::{
  create_api_token, list_tokens_for_user_scoped, revoke_token_scoped, CreateApiTokenInput,
};
use crate::auth::password::{hash_password, verify_password};
use crate::auth::principal::{resolve_principal, AuthScope};
use crate::auth::session::{create_session, destroy_session, find_session};
use crate::http::error::ApiError;
use crate::tenancy::spaces::ensure_personal_space;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
  Router::new()
    .route("/api/auth/register", post(register))
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

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
  pub username: String,
  pub password: String,
}

fn session_cookie(session_id: &str) -> Result<HeaderValue, ApiError> {
  HeaderValue::from_str(&format!(
    "knowledge_session={session_id}; HttpOnly; Path=/; SameSite=Lax"
  ))
  .map_err(|_| ApiError::internal("failed to build session cookie"))
}

async fn register(
  State(state): State<AppState>,
  Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let username = payload.username.trim().to_string();
  if username.len() < 3 || username.len() > 50 {
    return Err(ApiError::bad_request("username must be 3-50 characters"));
  }
  if payload.password.len() < 8 {
    return Err(ApiError::bad_request("password must be at least 8 characters"));
  }

  let password_hash = hash_password(&payload.password)?;
  let user_id = Uuid::new_v4().to_string();
  let now = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .unwrap_or_else(|_| String::from("1970-01-01T00:00:00Z"));

  // Rely on the users.username UNIQUE index instead of a racy COUNT precheck:
  // a duplicate raises SQLSTATE 23505, which from_db_unique maps to 400.
  sqlx::query(
    "INSERT INTO users (id, username, password_hash, role, created_at)
     VALUES ($1, $2, $3, $4, $5)",
  )
  .bind(&user_id)
  .bind(&username)
  .bind(&password_hash)
  .bind("user")
  .bind(&now)
  .execute(&state.pool)
  .await
  .map_err(|error| ApiError::from_db_unique(error, "username already taken"))?;

  ensure_personal_space(&state.pool, &user_id, &now)
    .await
    .map_err(ApiError::from)?;

  let session = create_session(&state, &user_id).await?;
  let mut response = (
    StatusCode::CREATED,
    Json(LoginResponse {
      csrf_token: session.csrf_token.clone(),
    }),
  )
    .into_response();
  response
    .headers_mut()
    .append(header::SET_COOKIE, session_cookie(&session.id)?);

  Ok(response)
}

async fn login(
  State(state): State<AppState>,
  Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let user = sqlx::query_as::<_, (String, String, String)>(
    "SELECT id, username, password_hash FROM users WHERE username = $1",
  )
  .bind(&payload.username)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?
  .ok_or_else(|| ApiError::unauthorized("invalid credentials"))?;

  if !verify_password(&payload.password, &user.2)? {
    return Err(ApiError::unauthorized("invalid credentials"));
  }

  let session = create_session(&state, &user.0).await?;
  let mut response = Json(LoginResponse {
    csrf_token: session.csrf_token.clone(),
  })
  .into_response();

  response
    .headers_mut()
    .append(header::SET_COOKIE, session_cookie(&session.id)?);

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
  // Scope containment: project-scoped Bearer principals can only mint
  // tokens for their own scope, never null and never another project.
  if principal.scope == AuthScope::ApiToken
    && let Some(scope) = principal.project_id.as_deref()
  {
    let requested = payload.project_id.as_deref();
    if requested != Some(scope) {
      return Err(ApiError::forbidden(
        "project-scoped api tokens can only mint tokens for the same project",
      ));
    }
  }
  if let Some(project_id) = payload.project_id.as_deref() {
    let role = crate::tenancy::access::project_access_role(
      &state.pool,
      project_id,
      &principal.user_id,
    )
    .await
    .map_err(ApiError::from)?;
    if role.is_none() {
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
  let scope = if principal.scope == AuthScope::ApiToken {
    principal.project_id.as_deref()
  } else {
    None
  };
  let tokens = list_tokens_for_user_scoped(&state, &principal.user_id, scope).await?;
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
  let scope = if principal.scope == AuthScope::ApiToken {
    principal.project_id.as_deref()
  } else {
    None
  };
  let revoked = revoke_token_scoped(&state, &principal.user_id, scope, &token_id).await?;
  if !revoked {
    return Err(ApiError::not_found("token not found"));
  }
  Ok(Json(json!({ "revoked": true })))
}
