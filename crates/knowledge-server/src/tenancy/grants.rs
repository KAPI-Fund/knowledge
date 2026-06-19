use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::routes::{authorized_principal, validate_csrf};
use crate::tenancy::access::{can_manage_kb_access, org_member_role, team_member_role};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/projects/{project_id}/grants", post(upsert_grant_handler))
        .route(
            "/api/projects/{project_id}/grants/{user_id}",
            axum::routing::delete(delete_grant_handler),
        )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UpsertGrantRequest {
    user_id: String,
    role: String,
}

/// Look up the owning (kind, org_id, team_id) for a project's space.
async fn project_space(
    pool: &sqlx::PgPool,
    project_id: &str,
) -> Result<(String, Option<String>, Option<String>), ApiError> {
    sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT s.kind, s.org_id, s.team_id \
         FROM spaces s JOIN projects p ON p.space_id = s.id \
         WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)?
    .ok_or_else(|| ApiError::not_found("project not found"))
}

async fn upsert_grant_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(payload): Json<UpsertGrantRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;

    if payload.role != "editor" && payload.role != "viewer" {
        return Err(ApiError::bad_request("role must be editor or viewer"));
    }
    if !can_manage_kb_access(&state.pool, &project_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
    {
        return Err(ApiError::forbidden("not permitted to manage access"));
    }

    let (kind, org_id, team_id) = project_space(&state.pool, &project_id).await?;
    let owning_org = match kind.as_str() {
        "org" => org_id.ok_or_else(|| ApiError::internal("org space missing org_id"))?,
        "team" => {
            let team_id = team_id.ok_or_else(|| ApiError::internal("team space missing team_id"))?;
            if team_member_role(&state.pool, &team_id, &payload.user_id)
                .await
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::bad_request("user is not a member of the team"));
            }
            sqlx::query_scalar::<_, String>("SELECT org_id FROM teams WHERE id = $1")
                .bind(&team_id)
                .fetch_one(&state.pool)
                .await
                .map_err(ApiError::from)?
        }
        _ => return Err(ApiError::bad_request("cannot grant on this space")),
    };
    if org_member_role(&state.pool, &owning_org, &payload.user_id)
        .await
        .map_err(ApiError::from)?
        .is_none()
    {
        return Err(ApiError::bad_request("user is not a member of the org"));
    }

    let can_import = payload.role == "editor";
    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    sqlx::query(
        "INSERT INTO project_members (id, project_id, user_id, role, can_import, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (project_id, user_id) \
         DO UPDATE SET role = EXCLUDED.role, can_import = EXCLUDED.can_import",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&project_id)
    .bind(&payload.user_id)
    .bind(&payload.role)
    .bind(can_import)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "projectId": project_id,
            "userId": payload.user_id,
            "role": payload.role,
            "canImport": can_import,
        })),
    ))
}

async fn delete_grant_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, user_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;
    if !can_manage_kb_access(&state.pool, &project_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
    {
        return Err(ApiError::forbidden("not permitted to manage access"));
    }
    sqlx::query("DELETE FROM project_members WHERE project_id = $1 AND user_id = $2")
        .bind(&project_id)
        .bind(&user_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}
