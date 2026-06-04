use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::app::state::AppState;
use crate::auth::session::find_session;
use crate::http::error::ApiError;
use crate::projects::service::{create_project, project_root_for_id};
use knowledge_core::graph::build_graph;
use knowledge_core::ingest::{analyze_source, generate_wiki_from_analysis};
use knowledge_core::project::sources::{
  delete_source, import_source, list_sources, load_queue, rescan_sources,
};
use knowledge_core::query::answer_from_results;
use knowledge_core::search::search_project;

pub fn router() -> Router<AppState> {
  Router::new()
    .route("/api/projects", get(list_projects_handler).post(create_project_handler))
    .route("/api/projects/{project_id}/members", get(list_project_members))
    .route("/api/projects/{project_id}/sources", get(list_sources_handler))
    .route("/api/projects/{project_id}/sources:import", post(import_source_handler))
    .route("/api/projects/{project_id}/sources:rescan", post(rescan_sources_handler))
    .route("/api/projects/{project_id}/sources/{*relative_path}", delete(delete_source_handler))
    .route("/api/projects/{project_id}/search", post(search_handler))
    .route("/api/projects/{project_id}/graph", get(graph_handler))
    .route("/api/projects/{project_id}/tasks", get(tasks_handler))
    .route("/api/projects/{project_id}/ingest", post(ingest_handler))
    .route("/api/projects/{project_id}/query", post(query_handler))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
  pub name: String,
  pub root_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSourceRequest {
  pub file_name: String,
  pub content_base64: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchRequest {
  pub query: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestRequest {
  pub relative_path: String,
}

async fn create_project_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, None).await?;
  validate_csrf(&headers, &session.csrf_token)?;
  let project = create_project(&payload, &state, &session.user_id).await?;
  Ok((StatusCode::CREATED, Json(project)))
}

async fn list_projects_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, None).await?;
  let rows = sqlx::query_as::<_, (String, String, String, String)>(
    "SELECT id, name, root_path, created_at FROM projects ORDER BY created_at ASC",
  )
  .fetch_all(&state.pool)
  .await
  .map_err(ApiError::from)?;

  let projects = rows
    .into_iter()
    .map(|(id, name, root_path, created_at)| {
      json!({
        "id": id,
        "name": name,
        "rootPath": root_path,
        "createdAt": created_at
      })
    })
    .collect::<Vec<_>>();

  Ok(Json(json!({ "projects": projects })))
}

async fn list_project_members(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;

  let rows = sqlx::query_as::<_, (String, String, i64)>(
    "SELECT user_id, role, can_import FROM project_members WHERE project_id = ?1 ORDER BY created_at ASC",
  )
  .bind(&project_id)
  .fetch_all(&state.pool)
  .await
  .map_err(ApiError::from)?;

  let members = rows
    .into_iter()
    .map(|(user_id, role, can_import)| {
      json!({
        "userId": user_id,
        "role": role,
        "canImport": can_import != 0
      })
    })
    .collect::<Vec<_>>();

  Ok(Json(json!({ "members": members })))
}

async fn list_sources_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let root = project_root_for_id(&state, &project_id).await?;
  let sources = list_sources(&root).map_err(|error| ApiError::bad_request(error.to_string()))?;
  Ok(Json(json!({ "sources": sources })))
}

async fn import_source_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Json(payload): Json<ImportSourceRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;
  let root = project_root_for_id(&state, &project_id).await?;
  let source =
    import_source(&root, &payload.file_name, &payload.content_base64).map_err(|error| {
      ApiError::bad_request(error.to_string())
    })?;
  Ok((StatusCode::CREATED, Json(source)))
}

async fn rescan_sources_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;
  let root = project_root_for_id(&state, &project_id).await?;
  let discovered_count =
    rescan_sources(&root).map_err(|error| ApiError::bad_request(error.to_string()))?;
  Ok(Json(json!({ "discoveredCount": discovered_count })))
}

async fn delete_source_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((project_id, relative_path)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;
  let root = project_root_for_id(&state, &project_id).await?;
  delete_source(&root, &relative_path).map_err(|error| ApiError::bad_request(error.to_string()))?;
  Ok(StatusCode::NO_CONTENT)
}

async fn search_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Json(payload): Json<SearchRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let root = project_root_for_id(&state, &project_id).await?;
  let results = search_project(root.as_path(), &payload.query)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  Ok(Json(json!({ "results": results })))
}

async fn graph_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let root = project_root_for_id(&state, &project_id).await?;
  let (nodes, edges) =
    build_graph(root.as_path()).map_err(|error| ApiError::bad_request(error.to_string()))?;
  Ok(Json(json!({ "nodes": nodes, "edges": edges })))
}

async fn tasks_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let root = project_root_for_id(&state, &project_id).await?;
  let queue = load_queue(&root).map_err(|error| ApiError::bad_request(error.to_string()))?;
  Ok(Json(json!({ "tasks": queue.tasks })))
}

async fn ingest_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Json(payload): Json<IngestRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;
  let root = project_root_for_id(&state, &project_id).await?;
  let source_path = root
    .safe_join(&payload.relative_path)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  let content = std::fs::read_to_string(&source_path).map_err(|error| ApiError::bad_request(error.to_string()))?;
  let source_name = source_path
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| ApiError::bad_request("invalid source name"))?;
  let analysis = analyze_source(source_name, &content);
  let result = generate_wiki_from_analysis(&root, source_name, &analysis)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  Ok(Json(json!(result)))
}

async fn query_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Json(payload): Json<SearchRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let root = project_root_for_id(&state, &project_id).await?;
  let results = search_project(root.as_path(), &payload.query)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  let answer = answer_from_results(&payload.query, &results);
  Ok(Json(json!(answer)))
}

fn extract_session_id(headers: &HeaderMap) -> Option<String> {
  headers
    .get(header::COOKIE)
    .and_then(|value| value.to_str().ok())
    .and_then(|cookie| {
      cookie
        .split(';')
        .map(str::trim)
        .find(|item| item.starts_with("knowledge_session="))
        .map(|item| item.trim_start_matches("knowledge_session=").to_string())
    })
}

fn validate_csrf(headers: &HeaderMap, expected_token: &str) -> Result<(), ApiError> {
  let supplied = headers
    .get("x-csrf-token")
    .and_then(|value| value.to_str().ok())
    .unwrap_or_default();

  if supplied.is_empty() || supplied != expected_token {
    return Err(ApiError::unauthorized("invalid csrf token"));
  }

  Ok(())
}

async fn authorized_session(
  state: &AppState,
  headers: &HeaderMap,
  project_id: Option<&str>,
) -> Result<crate::auth::session::SessionRecord, ApiError> {
  let session_id = extract_session_id(headers).ok_or_else(|| ApiError::unauthorized("missing session"))?;
  let session = find_session(state, &session_id)
    .await?
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;

  if let Some(project_id) = project_id {
    let membership = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM project_members WHERE project_id = ?1 AND user_id = ?2",
    )
    .bind(project_id)
    .bind(&session.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

    if membership == 0 {
      return Err(ApiError::forbidden("not a project member"));
    }
  }

  Ok(session)
}
