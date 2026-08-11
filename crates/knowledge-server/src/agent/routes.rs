use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde_json::json;

use crate::agent::skills::{list_available_skills, load_skills};
use crate::app::state::AppState;
use crate::chat::store::find_conversation;
use crate::http::error::ApiError;
use crate::projects::routes::{authorized_principal, validate_csrf};
use crate::projects::service::project_root_for_id;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/projects/{project_id}/agent/skills",
            get(list_skills_handler),
        )
        .route(
            "/api/projects/{project_id}/agent/skills/{skill_id}",
            get(get_skill_handler),
        )
        .route(
            "/api/projects/{project_id}/conversations/{conversation_id}/messages/active",
            delete(cancel_active_run_handler),
        )
}

async fn list_skills_handler(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    authorized_principal(&state, &headers, Some(&project_id)).await?;
    let root = project_root_for_id(&state, &project_id).await?;
    let project_path = root.as_path().to_path_buf();
    let global_dir = state.global_skills_dir.clone();
    let skills = tokio::task::spawn_blocking(move || {
        list_available_skills(&project_path, global_dir.as_deref())
    })
    .await
    .map_err(|_| ApiError::internal("failed to scan agent skills"))?;
    Ok(Json(json!({ "skills": skills })))
}

async fn get_skill_handler(
    State(state): State<AppState>,
    Path((project_id, skill_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    authorized_principal(&state, &headers, Some(&project_id)).await?;
    let root = project_root_for_id(&state, &project_id).await?;
    let project_path = root.as_path().to_path_buf();
    let global_dir = state.global_skills_dir.clone();
    let skill = tokio::task::spawn_blocking(move || {
        let listing = list_available_skills(&project_path, global_dir.as_deref())
            .into_iter()
            .find(|entry| entry.id == skill_id)?;
        let loaded = load_skills(&project_path, global_dir.as_deref(), &[skill_id.clone()])
            .into_iter()
            .next()?;
        // AgentSkill.location/base_dir hold absolute server paths; expose only
        // the portable id, metadata, and instructions body.
        Some(json!({
            "id": listing.id,
            "name": loaded.name,
            "description": loaded.description,
            "instructions": loaded.instructions,
            "source": listing.source,
        }))
    })
    .await
    .map_err(|_| ApiError::internal("failed to read agent skill"))?
    .ok_or_else(|| ApiError::not_found("agent skill not found"))?;
    Ok(Json(skill))
}

async fn cancel_active_run_handler(
    State(state): State<AppState>,
    Path((project_id, conversation_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_principal(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session)?;
    find_conversation(&state.pool, &project_id, &conversation_id, &session.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("conversation not found"))?;
    let cancelled = state
        .agent_cancellations
        .cancel(&project_id, &conversation_id, None);
    if !cancelled {
        return Err(ApiError::not_found(
            "no active agent run for this conversation",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}
