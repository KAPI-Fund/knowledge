use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::app::state::AppState;
use crate::auth::session::find_session;
use crate::http::error::ApiError;
use crate::projects::audit::{append_audit_log, list_audit_logs, CreateAuditLog};
use crate::projects::service::{create_project, project_detail, project_root_for_id};
use crate::projects::tasks::{
  create_queued_task, get_task, list_tasks, update_task_status,
  CreateTaskRecord,
};
use crate::retrieval::service::search_project_hybrid;
use knowledge_core::graph::{build_graph_view, neighbors_for_node};
use knowledge_core::project::files::{
  clamp_max_files, list_project_files, parse_project_file_root, read_project_file_content,
  ProjectFileListOptions, ProjectFilesError,
};
use knowledge_core::project::reviews::load_reviews;
use knowledge_core::project::sources::list_sources;
use knowledge_core::query::answer_from_results;
use knowledge_core::search::SearchOptions;

pub fn router() -> Router<AppState> {
  Router::new()
    .route("/api/projects", get(list_projects_handler).post(create_project_handler))
    .route("/api/projects/{project_id}", get(project_detail_handler))
    .route("/api/projects/{project_id}/members", get(list_project_members))
    .route("/api/projects/{project_id}/sources", get(list_sources_handler))
    .route("/api/projects/{project_id}/sources:import", post(import_source_handler))
    .route("/api/projects/{project_id}/sources:rescan", post(rescan_sources_handler))
    .route("/api/projects/{project_id}/sources/{*relative_path}", delete(delete_source_handler))
    .route("/api/projects/{project_id}/files", get(list_files_handler))
    .route("/api/projects/{project_id}/files/content", get(file_content_handler))
    .route("/api/projects/{project_id}/search", post(search_handler))
    .route("/api/projects/{project_id}/graph", get(graph_handler))
    .route("/api/projects/{project_id}/graph/{node_id}/neighbors", get(graph_neighbors_handler))
    .route("/api/projects/{project_id}/tasks", get(tasks_handler))
    .route("/api/projects/{project_id}/tasks/{task_id}", get(task_detail_handler))
    .route("/api/projects/{project_id}/tasks/{task_id}/retry", post(retry_task_handler))
    .route("/api/projects/{project_id}/tasks/{task_id}/cancel", post(cancel_task_handler))
    .route("/api/projects/{project_id}/query-tasks", post(create_query_task_handler))
    .route("/api/projects/{project_id}/query-tasks/{task_id}", get(query_task_detail_handler))
    .route("/api/projects/{project_id}/query-tasks/{task_id}/save", post(save_query_task_handler))
    .route("/api/projects/{project_id}/ingest", post(ingest_handler))
    .route("/api/projects/{project_id}/query", post(query_handler))
    .route("/api/projects/{project_id}/reviews", get(list_reviews_handler))
    .route("/api/projects/{project_id}/reviews:sweep", post(sweep_reviews_handler))
    .route("/api/projects/{project_id}/reviews/{review_id}", patch(update_review_handler))
    .route("/api/projects/{project_id}/audit-logs", get(audit_logs_handler))
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
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
  pub query: String,
  #[serde(default)]
  pub top_k: Option<usize>,
  #[serde(default)]
  pub include_content: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileListRequest {
  #[serde(default)]
  pub root: Option<String>,
  #[serde(default)]
  pub recursive: Option<bool>,
  #[serde(default)]
  pub max_files: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileContentRequest {
  pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateQueryTaskRequest {
  pub query: String,
  pub top_k: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveQueryTaskRequest {
  pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestRequest {
  pub relative_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateReviewRequest {
  pub status: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GraphRequest {
  #[serde(default, rename = "q")]
  pub query: Option<String>,
  #[serde(default, rename = "nodeType")]
  pub node_type: Option<String>,
  #[serde(default)]
  pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReviewListRequest {
  #[serde(default)]
  pub status: Option<String>,
  #[serde(default, rename = "type")]
  pub item_type: Option<String>,
  #[serde(default)]
  pub limit: Option<usize>,
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

async fn project_detail_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  Ok(Json(project_detail(&state, &project_id).await?))
}

async fn list_project_members(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;

  let rows = sqlx::query_as::<_, (String, String, bool)>(
    "SELECT user_id, role, can_import FROM project_members WHERE project_id = $1 ORDER BY created_at ASC",
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
        "canImport": can_import
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
  let task = create_queued_task(
    &state,
    CreateTaskRecord {
      project_id: project_id.clone(),
      task_type: "project.import_source".to_string(),
      title: format!("Imported {}", payload.file_name),
      relative_path: Some(format!("raw/sources/{}", payload.file_name)),
      detail: json!({}),
      created_by: session.user_id.clone(),
    },
    json!({
      "fileName": payload.file_name,
      "contentBase64": payload.content_base64
    }),
  )
  .await?;
  append_audit_log(
    &state,
    CreateAuditLog {
      project_id: Some(project_id.clone()),
      actor_id: session.user_id.clone(),
      action: "source.import.enqueued".to_string(),
      target_type: "source".to_string(),
      target_id: payload.file_name.clone(),
      task_id: Some(task.id.clone()),
      summary: format!("Queued source import {}", payload.file_name),
      metadata: json!({ "fileName": payload.file_name }),
    },
  )
  .await?;
  Ok((
    StatusCode::ACCEPTED,
    Json(json!({
      "taskId": task.id,
      "status": task.status
    })),
  ))
}

async fn rescan_sources_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;
  let task = create_queued_task(
    &state,
    CreateTaskRecord {
      project_id: project_id.clone(),
      task_type: "project.rescan_sources".to_string(),
      title: "Rescanned sources".to_string(),
      relative_path: None,
      detail: json!({}),
      created_by: session.user_id.clone(),
    },
    json!({}),
  )
  .await?;
  append_audit_log(
    &state,
    CreateAuditLog {
      project_id: Some(project_id.clone()),
      actor_id: session.user_id.clone(),
      action: "sources.rescan.enqueued".to_string(),
      target_type: "project".to_string(),
      target_id: project_id.clone(),
      task_id: Some(task.id.clone()),
      summary: "Queued project source rescan".to_string(),
      metadata: json!({}),
    },
  )
  .await?;
  Ok((
    StatusCode::ACCEPTED,
    Json(json!({
      "taskId": task.id,
      "status": task.status
    })),
  ))
}

async fn delete_source_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((project_id, relative_path)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;
  let task = create_queued_task(
    &state,
    CreateTaskRecord {
      project_id: project_id.clone(),
      task_type: "project.delete_source".to_string(),
      title: format!("Deleted {}", relative_path),
      relative_path: Some(format!("raw/sources/{relative_path}")),
      detail: json!({}),
      created_by: session.user_id.clone(),
    },
    json!({
      "relativePath": relative_path
    }),
  )
  .await?;
  append_audit_log(
    &state,
    CreateAuditLog {
      project_id: Some(project_id.clone()),
      actor_id: session.user_id.clone(),
      action: "source.delete.enqueued".to_string(),
      target_type: "source".to_string(),
      target_id: relative_path.clone(),
      task_id: Some(task.id.clone()),
      summary: format!("Queued source delete {}", relative_path),
      metadata: json!({ "relativePath": relative_path }),
    },
  )
  .await?;
  Ok((
    StatusCode::ACCEPTED,
    Json(json!({
      "taskId": task.id,
      "status": task.status
    })),
  ))
}

async fn list_files_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Query(params): Query<FileListRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let root = project_root_for_id(&state, &project_id).await?;
  let options = ProjectFileListOptions {
    root: parse_project_file_root(params.root.as_deref()).map_err(map_project_files_error)?,
    recursive: params.recursive.unwrap_or(true),
    max_files: clamp_max_files(params.max_files),
  };
  let listing = list_project_files(&root, &options).map_err(map_project_files_error)?;
  Ok(Json(json!({
    "root": listing.root,
    "files": listing.files,
    "truncated": listing.truncated
  })))
}

async fn file_content_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Query(params): Query<FileContentRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let root = project_root_for_id(&state, &project_id).await?;
  let file = read_project_file_content(&root, &params.path).map_err(map_project_files_error)?;
  Ok(Json(json!({
    "path": file.path,
    "content": file.content
  })))
}

async fn search_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Json(payload): Json<SearchRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let root = project_root_for_id(&state, &project_id).await?;
  let response = search_project_hybrid(
    &state,
    &project_id,
    &root,
    &payload.query,
    SearchOptions {
      top_k: payload.top_k.unwrap_or(10),
      include_content: payload.include_content.unwrap_or(false),
    },
  )
  .await?;
  Ok(Json(json!({
    "mode": response.mode,
    "tokenHits": response.token_hits,
    "vectorHits": response.vector_hits,
    "results": response.results
  })))
}

async fn graph_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Query(params): Query<GraphRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let root = project_root_for_id(&state, &project_id).await?;
  let (nodes, edges) = build_graph_view(
    root.as_path(),
    params.query.as_deref(),
    params.node_type.as_deref(),
    params.limit,
  )
  .map_err(|error| ApiError::bad_request(error.to_string()))?;
  Ok(Json(json!({ "nodes": nodes, "edges": edges })))
}

async fn graph_neighbors_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((project_id, node_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let root = project_root_for_id(&state, &project_id).await?;
  let neighborhood = neighbors_for_node(root.as_path(), &node_id)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  Ok(Json(json!(neighborhood)))
}

async fn tasks_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let tasks = list_tasks(&state, &project_id).await?;
  Ok(Json(json!({ "tasks": tasks })))
}

async fn task_detail_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((project_id, task_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  Ok(Json(json!(get_task(&state, &project_id, &task_id).await?)))
}

async fn retry_task_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((project_id, task_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;
  let updated = update_task_status(&state, &project_id, &task_id, "queued").await?;
  Ok(Json(json!(updated)))
}

async fn cancel_task_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((project_id, task_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;
  let current = get_task(&state, &project_id, &task_id).await?;
  if current.status == "completed" {
    return Err(ApiError::bad_request("completed tasks cannot be cancelled"));
  }
  let updated = update_task_status(&state, &project_id, &task_id, "cancelled").await?;
  Ok(Json(json!(updated)))
}

async fn create_query_task_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Json(payload): Json<CreateQueryTaskRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;

  let (provider_mode, language, default_query_limit) =
    sqlx::query_as::<_, (String, String, i64)>(
      "SELECT provider_mode, language, default_query_limit FROM system_settings WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

  let top_k = if payload.top_k > 0 {
    payload.top_k
  } else {
    default_query_limit
  };

  let task = create_queued_task(
    &state,
    CreateTaskRecord {
      project_id: project_id.clone(),
      task_type: "query.answer".to_string(),
      title: format!("Query: {}", payload.query),
      relative_path: None,
      detail: json!({}),
      created_by: session.user_id.clone(),
    },
    json!({
      "query": payload.query,
      "topK": top_k,
      "providerMode": provider_mode,
      "language": language,
      "requestedBy": session.user_id
    }),
  )
  .await?;

  Ok((
    StatusCode::ACCEPTED,
    Json(json!({
      "taskId": task.id,
      "status": task.status
    })),
  ))
}

async fn query_task_detail_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((project_id, task_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  Ok(Json(json!(get_task(&state, &project_id, &task_id).await?)))
}

async fn save_query_task_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((project_id, task_id)): Path<(String, String)>,
  Json(payload): Json<SaveQueryTaskRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;

  let source_task = get_task(&state, &project_id, &task_id).await?;
  if source_task.task_type != "query.answer" || source_task.status != "succeeded" {
    return Err(ApiError::bad_request("only successful query answer tasks can be saved"));
  }

  let result = source_task
    .result
    .as_ref()
    .ok_or_else(|| ApiError::bad_request("query task result is missing"))?;
  let title = payload.title.trim();
  if title.is_empty() {
    return Err(ApiError::bad_request("title is required"));
  }
  let slug = slugify_title(title);
  if slug.is_empty() {
    return Err(ApiError::bad_request("title does not produce a valid slug"));
  }
  let answer = result
    .get("answer")
    .and_then(serde_json::Value::as_str)
    .ok_or_else(|| ApiError::bad_request("query answer is missing"))?;
  let context_summary = result
    .get("contextSummary")
    .and_then(serde_json::Value::as_str)
    .unwrap_or_default();
  let citations = result
    .get("citations")
    .and_then(serde_json::Value::as_array)
    .cloned()
    .unwrap_or_default();

  let task = create_queued_task(
    &state,
    CreateTaskRecord {
      project_id: project_id.clone(),
      task_type: "query.save_answer".to_string(),
      title: format!("Save query: {title}"),
      relative_path: Some(format!("wiki/queries/{slug}.md")),
      detail: json!({
        "sourceTaskId": task_id
      }),
      created_by: session.user_id.clone(),
    },
    json!({
      "sourceTaskId": task_id,
      "title": title,
      "slug": slug,
      "answer": answer,
      "citations": citations,
      "contextSummary": context_summary
    }),
  )
  .await?;

  append_audit_log(
    &state,
    CreateAuditLog {
      project_id: Some(project_id),
      actor_id: session.user_id,
      action: "query.save.enqueued".to_string(),
      target_type: "query".to_string(),
      target_id: task_id,
      task_id: Some(task.id.clone()),
      summary: format!("Queued save-to-wiki for {title}"),
      metadata: json!({
        "slug": slug
      }),
    },
  )
  .await?;

  Ok((
    StatusCode::ACCEPTED,
    Json(json!({
      "taskId": task.id,
      "status": task.status
    })),
  ))
}

async fn ingest_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Json(payload): Json<IngestRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;
  let task = create_queued_task(
    &state,
    CreateTaskRecord {
      project_id: project_id.clone(),
      task_type: "project.ingest_source".to_string(),
      title: format!("Ingest {}", payload.relative_path),
      relative_path: Some(payload.relative_path.clone()),
      detail: json!({}),
      created_by: session.user_id.clone(),
    },
    json!({
      "relativePath": payload.relative_path
    }),
  )
  .await?;
  append_audit_log(
    &state,
    CreateAuditLog {
      project_id: Some(project_id.clone()),
      actor_id: session.user_id.clone(),
      action: "ingest.enqueued".to_string(),
      target_type: "source".to_string(),
      target_id: payload.relative_path.clone(),
      task_id: Some(task.id.clone()),
      summary: format!("Queued ingest for {}", payload.relative_path),
      metadata: json!({ "relativePath": payload.relative_path }),
    },
  )
  .await?;
  Ok((
    StatusCode::ACCEPTED,
    Json(json!({
      "taskId": task.id,
      "status": task.status
    })),
  ))
}

async fn query_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Json(payload): Json<SearchRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let root = project_root_for_id(&state, &project_id).await?;
  let results = search_project_hybrid(
    &state,
    &project_id,
    &root,
    &payload.query,
    SearchOptions::default(),
  )
  .await?;
  let answer = answer_from_results(&payload.query, &results.results);
  Ok(Json(json!(answer)))
}

async fn list_reviews_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Query(params): Query<ReviewListRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let root = project_root_for_id(&state, &project_id).await?;
  let store = load_reviews(&root).map_err(|error| ApiError::bad_request(error.to_string()))?;
  let status = parse_review_status(params.status.as_deref())?;
  let item_type = params
    .item_type
    .as_deref()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_string);
  let limit = params.limit.unwrap_or(200).clamp(1, 1_000);

  let reviews = store
    .reviews
    .into_iter()
    .filter(|review| review_status_matches(status, &review.status))
    .filter(|review| {
      item_type
        .as_deref()
        .map(|expected| review.review_type == expected)
        .unwrap_or(true)
    })
    .take(limit)
    .collect::<Vec<_>>();

  Ok(Json(json!({ "reviews": reviews })))
}

async fn sweep_reviews_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;
  let task = create_queued_task(
    &state,
    CreateTaskRecord {
      project_id: project_id.clone(),
      task_type: "project.sweep_reviews".to_string(),
      title: "Sweep reviews".to_string(),
      relative_path: None,
      detail: json!({}),
      created_by: session.user_id.clone(),
    },
    json!({}),
  )
  .await?;
  append_audit_log(
    &state,
    CreateAuditLog {
      project_id: Some(project_id),
      actor_id: session.user_id,
      action: "review.sweep.enqueued".to_string(),
      target_type: "review".to_string(),
      target_id: "batch".to_string(),
      task_id: Some(task.id.clone()),
      summary: "Queued review sweep".to_string(),
      metadata: json!({}),
    },
  )
  .await?;
  Ok((
    StatusCode::ACCEPTED,
    Json(json!({
      "taskId": task.id,
      "status": task.status
    })),
  ))
}

async fn update_review_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path((project_id, review_id)): Path<(String, String)>,
  Json(payload): Json<UpdateReviewRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let session = authorized_session(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &session.csrf_token)?;
  let task = create_queued_task(
    &state,
    CreateTaskRecord {
      project_id: project_id.clone(),
      task_type: "project.update_review".to_string(),
      title: format!("Update review {}", review_id),
      relative_path: None,
      detail: json!({}),
      created_by: session.user_id.clone(),
    },
    json!({
      "reviewId": review_id,
      "status": payload.status
    }),
  )
  .await?;
  append_audit_log(
    &state,
    CreateAuditLog {
      project_id: Some(project_id),
      actor_id: session.user_id,
      action: "review.update.enqueued".to_string(),
      target_type: "review".to_string(),
      target_id: review_id,
      task_id: Some(task.id.clone()),
      summary: format!("Queued review status update to {}", payload.status),
      metadata: json!({ "status": payload.status }),
    },
  )
  .await?;
  Ok((
    StatusCode::ACCEPTED,
    Json(json!({
      "taskId": task.id,
      "status": task.status
    })),
  ))
}

async fn audit_logs_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
  let _session = authorized_session(&state, &headers, Some(&project_id)).await?;
  let items = list_audit_logs(&state, &project_id).await?;
  Ok(Json(json!({ "items": items })))
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

fn slugify_title(input: &str) -> String {
  let mut slug = String::new();
  let mut last_was_dash = false;

  for ch in input.chars() {
    if ch.is_ascii_alphanumeric() {
      slug.push(ch.to_ascii_lowercase());
      last_was_dash = false;
    } else if !last_was_dash && !slug.is_empty() {
      slug.push('-');
      last_was_dash = true;
    }
  }

  slug.trim_matches('-').to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewStatusFilter {
  Unresolved,
  Resolved,
  All,
}

fn parse_review_status(value: Option<&str>) -> Result<ReviewStatusFilter, ApiError> {
  match value.unwrap_or("unresolved") {
    "unresolved" | "pending" => Ok(ReviewStatusFilter::Unresolved),
    "resolved" => Ok(ReviewStatusFilter::Resolved),
    "all" => Ok(ReviewStatusFilter::All),
    invalid => Err(ApiError::bad_request(format!(
      "invalid review status '{invalid}'. expected unresolved, resolved, or all"
    ))),
  }
}

fn review_status_matches(filter: ReviewStatusFilter, status: &str) -> bool {
  match filter {
    ReviewStatusFilter::Unresolved => status != "resolved",
    ReviewStatusFilter::Resolved => status == "resolved",
    ReviewStatusFilter::All => true,
  }
}

fn map_project_files_error(error: ProjectFilesError) -> ApiError {
  match error {
    ProjectFilesError::InvalidRoot => ApiError::bad_request(error.to_string()),
    ProjectFilesError::NonPublicPath => ApiError::forbidden(error.to_string()),
    ProjectFilesError::NonTextPath | ProjectFilesError::InvalidUtf8 => {
      ApiError::unsupported_media_type(error.to_string())
    }
    ProjectFilesError::FileTooLarge | ProjectFilesError::ListingExceedsMaxFiles(_) => {
      ApiError::payload_too_large(error.to_string())
    }
    ProjectFilesError::NotFound => ApiError::not_found(error.to_string()),
    ProjectFilesError::Io(_) | ProjectFilesError::Root(_) => {
      ApiError::bad_request(error.to_string())
    }
  }
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
      "SELECT COUNT(*) FROM project_members WHERE project_id = $1 AND user_id = $2",
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
