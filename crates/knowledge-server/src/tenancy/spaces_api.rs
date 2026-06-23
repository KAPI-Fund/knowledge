use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::get;
use serde_json::json;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::auth::principal::require_session;
use crate::tenancy::spaces::personal_space_id;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/spaces", get(list_spaces_handler))
}

async fn list_spaces_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let principal = require_session(&state, &headers).await?;
    let user_id = &principal.user_id;

    let personal = personal_space_id(&state.pool, user_id)
        .await
        .map_err(ApiError::from)?;

    let org_rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT o.id, o.slug, o.name, s.id, om.role \
         FROM organization_members om \
         JOIN organizations o ON o.id = om.org_id \
         JOIN spaces s ON s.kind = 'org' AND s.org_id = o.id \
         WHERE om.user_id = $1 \
         ORDER BY o.created_at ASC",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::from)?;
    let orgs = org_rows
        .into_iter()
        .map(|(id, slug, name, space_id, role)| {
            json!({ "id": id, "slug": slug, "name": name, "spaceId": space_id, "role": role })
        })
        .collect::<Vec<_>>();

    let team_rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT t.id, t.org_id, t.slug, t.name, s.id, tm.role \
         FROM team_members tm \
         JOIN teams t ON t.id = tm.team_id \
         JOIN spaces s ON s.kind = 'team' AND s.team_id = t.id \
         WHERE tm.user_id = $1 \
         ORDER BY t.created_at ASC",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::from)?;
    let teams = team_rows
        .into_iter()
        .map(|(id, org_id, slug, name, space_id, role)| {
            json!({
                "id": id,
                "orgId": org_id,
                "slug": slug,
                "name": name,
                "spaceId": space_id,
                "role": role,
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(json!({
        "personal": { "spaceId": personal },
        "orgs": orgs,
        "teams": teams,
    })))
}
