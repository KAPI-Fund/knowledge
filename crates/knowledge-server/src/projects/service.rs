use knowledge_core::project::root::ProjectRoot;
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;

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
  let root = ProjectRoot::new(&input.root_path)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  let created_at = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format created_at"))?;

  sqlx::query(
    "INSERT INTO projects (id, name, root_path, created_at) VALUES (?1, ?2, ?3, ?4)",
  )
  .bind(&id)
  .bind(&input.name)
  .bind(root.as_str())
  .bind(&created_at)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  sqlx::query(
    "INSERT INTO project_members (id, project_id, user_id, role, can_import, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(&id)
  .bind(user_id)
  .bind("project_owner")
  .bind(1_i64)
  .bind(&created_at)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(ProjectDto {
    id,
    name: input.name.clone(),
    root_path: root.as_str().to_owned(),
    created_at,
  })
}
