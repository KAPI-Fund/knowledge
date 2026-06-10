use knowledge_core::project::root::ProjectRoot;
use knowledge_core::project::scaffold::initialize_project;
use serde::Serialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::audit::{CreateAuditLog, append_audit_log};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDto {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub created_at: String,
}

pub async fn create_project(
    input: &super::routes::CreateProjectRequest,
    state: &AppState,
    user_id: &str,
) -> Result<ProjectDto, ApiError> {
    let id = Uuid::new_v4().to_string();
    let root_path = PathBuf::from(&state.project_root).join(&id);
    let root =
        initialize_project(&root_path).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let root_path = normalize_project_path_string(root.as_path());
    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;

    sqlx::query("INSERT INTO projects (id, name, root_path, created_at) VALUES ($1, $2, $3, $4)")
        .bind(&id)
        .bind(&input.name)
        .bind(&root_path)
        .bind(&created_at)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;

    sqlx::query(
    "INSERT INTO project_members (id, project_id, user_id, role, can_import, created_at) VALUES ($1, $2, $3, $4, $5, $6)",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(&id)
  .bind(user_id)
  .bind("project_owner")
  .bind(true)
  .bind(&created_at)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

    append_audit_log(
        state,
        CreateAuditLog {
            project_id: Some(id.clone()),
            actor_id: user_id.to_string(),
            action: "project.created".to_string(),
            target_type: "project".to_string(),
            target_id: id.clone(),
            task_id: None,
            summary: format!("Created project {}", input.name),
            metadata: json!({
              "name": input.name,
              "rootPath": root_path
            }),
        },
    )
    .await?;

    Ok(ProjectDto {
        id,
        name: input.name.clone(),
        root_path,
        created_at,
    })
}

pub async fn project_root_for_id(
    state: &AppState,
    project_id: &str,
) -> Result<ProjectRoot, ApiError> {
    let root_path = sqlx::query_scalar::<_, String>("SELECT root_path FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::bad_request("unknown project"))?;

    ProjectRoot::new(root_path).map_err(|error| ApiError::bad_request(error.to_string()))
}

pub async fn project_detail(
    state: &AppState,
    project_id: &str,
) -> Result<serde_json::Value, ApiError> {
    let (id, name, root_path, created_at) = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, name, root_path, created_at FROM projects WHERE id = $1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::from)?
    .ok_or_else(|| ApiError::bad_request("unknown project"))?;

    let source_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM project_tasks WHERE project_id = $1 AND task_type = 'source_import'",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;
    let task_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM project_tasks WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::from)?;
    let review_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM audit_logs WHERE project_id = $1 AND action = 'review.updated'",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

    Ok(json!({
      "project": {
        "id": id,
        "name": name,
        "rootPath": normalize_project_path_string(&root_path),
        "createdAt": created_at,
        "sourceCount": source_count,
        "taskCount": task_count,
        "reviewCount": review_count
      }
    }))
}

pub fn normalize_project_path_string(path: impl AsRef<Path>) -> String {
    let value = path.as_ref().to_string_lossy().into_owned();
    normalize_windows_path_string(&value)
}

#[cfg(windows)]
fn normalize_windows_path_string(value: &str) -> String {
    if let Some(stripped) = value.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{}", stripped);
    }

    value
        .strip_prefix(r"\\?\")
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_owned())
}

#[cfg(not(windows))]
fn normalize_windows_path_string(value: &str) -> String {
    value.to_owned()
}
