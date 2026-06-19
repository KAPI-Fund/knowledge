use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::routes::{authorized_principal, validate_csrf};
use crate::tenancy::access::{is_org_admin, org_member_role, team_member_role};
use crate::tenancy::slug::validate_slug;
use crate::tenancy::spaces::create_team_space;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/orgs/{org_id}/teams",
            get(list_teams_handler).post(create_team_handler),
        )
        .route(
            "/api/orgs/{org_id}/teams/{team_id}/members",
            post(add_team_member_handler),
        )
        .route(
            "/api/orgs/{org_id}/teams/{team_id}/members/{user_id}",
            delete(remove_team_member_handler),
        )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateTeamRequest {
    name: String,
    slug: String,
}

async fn create_team_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org_id): Path<String>,
    Json(payload): Json<CreateTeamRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;
    if org_member_role(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
        .is_none()
    {
        return Err(ApiError::forbidden("not a member of this org"));
    }
    validate_slug(&payload.slug)?;

    let taken =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teams WHERE org_id = $1 AND slug = $2")
            .bind(&org_id)
            .bind(&payload.slug)
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::from)?;
    if taken > 0 {
        return Err(ApiError::bad_request("slug already taken in this org"));
    }

    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    let team_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO teams (id, org_id, name, slug, created_by, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&team_id)
    .bind(&org_id)
    .bind(&payload.name)
    .bind(&payload.slug)
    .bind(&principal.user_id)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    sqlx::query(
        "INSERT INTO team_members (id, team_id, user_id, role, created_at) \
         VALUES ($1, $2, $3, 'leader', $4)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&team_id)
    .bind(&principal.user_id)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let space_id = create_team_space(&state.pool, &team_id, &created_at)
        .await
        .map_err(ApiError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": team_id,
            "name": payload.name,
            "slug": payload.slug,
            "orgId": org_id,
            "spaceId": space_id,
        })),
    ))
}

async fn list_teams_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    let Some(role) = org_member_role(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
    else {
        return Err(ApiError::forbidden("not a member of this org"));
    };

    let rows = if role == "org_admin" {
        sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, name, slug FROM teams WHERE org_id = $1 ORDER BY created_at ASC",
        )
        .bind(&org_id)
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::from)?
    } else {
        sqlx::query_as::<_, (String, String, String)>(
            "SELECT t.id, t.name, t.slug FROM teams t \
             JOIN team_members tm ON tm.team_id = t.id \
             WHERE t.org_id = $1 AND tm.user_id = $2 ORDER BY t.created_at ASC",
        )
        .bind(&org_id)
        .bind(&principal.user_id)
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::from)?
    };

    let teams = rows
        .into_iter()
        .map(|(id, name, slug)| json!({ "id": id, "name": name, "slug": slug, "orgId": org_id }))
        .collect::<Vec<_>>();
    Ok(Json(json!({ "teams": teams })))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AddTeamMemberRequest {
    username_or_email: String,
}

async fn add_team_member_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((org_id, team_id)): Path<(String, String)>,
    Json(payload): Json<AddTeamMemberRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;

    let is_admin = is_org_admin(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?;
    let is_leader = team_member_role(&state.pool, &team_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
        .as_deref()
        == Some("leader");
    if !is_admin && !is_leader {
        return Err(ApiError::forbidden(
            "only org admins or the team leader may add members",
        ));
    }

    let belongs =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teams WHERE id = $1 AND org_id = $2")
            .bind(&team_id)
            .bind(&org_id)
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::from)?;
    if belongs == 0 {
        return Err(ApiError::not_found("team not found"));
    }

    let target = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = $1")
        .bind(&payload.username_or_email)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("user not found"))?;

    if org_member_role(&state.pool, &org_id, &target)
        .await
        .map_err(ApiError::from)?
        .is_none()
    {
        return Err(ApiError::bad_request("user is not a member of the org"));
    }

    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_members WHERE team_id = $1 AND user_id = $2",
    )
    .bind(&team_id)
    .bind(&target)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;
    if existing > 0 {
        return Err(ApiError::bad_request("user is already a team member"));
    }

    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    sqlx::query(
        "INSERT INTO team_members (id, team_id, user_id, role, created_at) \
         VALUES ($1, $2, $3, 'member', $4)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&team_id)
    .bind(&target)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "teamId": team_id, "userId": target, "role": "member" })),
    ))
}

async fn remove_team_member_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((org_id, team_id, user_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;

    let is_admin = is_org_admin(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?;
    let is_leader = team_member_role(&state.pool, &team_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
        .as_deref()
        == Some("leader");
    if !is_admin && !is_leader {
        return Err(ApiError::forbidden(
            "only org admins or the team leader may remove members",
        ));
    }

    let target_role = team_member_role(&state.pool, &team_id, &user_id)
        .await
        .map_err(ApiError::from)?;
    if target_role.as_deref() == Some("leader") {
        return Err(ApiError::bad_request("cannot remove the team leader"));
    }

    sqlx::query("DELETE FROM team_members WHERE team_id = $1 AND user_id = $2")
        .bind(&team_id)
        .bind(&user_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}
