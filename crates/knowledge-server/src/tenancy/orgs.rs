use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, post};
use serde::Deserialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::routes::{authorized_principal, validate_csrf};
use crate::tenancy::access::{is_org_admin, org_member_role};
use crate::tenancy::slug::validate_slug;
use crate::tenancy::spaces::create_org_space;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/orgs", post(create_org_handler))
        .route(
            "/api/orgs/{org_id}/members",
            post(add_org_member_handler).get(list_org_members_handler),
        )
        .route(
            "/api/orgs/{org_id}/members/{user_id}",
            delete(remove_org_member_handler).patch(set_org_member_role_handler),
        )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateOrgRequest {
    name: String,
    slug: String,
}

async fn create_org_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateOrgRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;
    validate_slug(&payload.slug)?;

    let taken = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organizations WHERE slug = $1")
        .bind(&payload.slug)
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::from)?;
    if taken > 0 {
        return Err(ApiError::bad_request("slug already taken"));
    }

    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    let org_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO organizations (id, name, slug, created_by, created_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&org_id)
    .bind(&payload.name)
    .bind(&payload.slug)
    .bind(&principal.user_id)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    sqlx::query(
        "INSERT INTO organization_members (id, org_id, user_id, role, created_at) \
         VALUES ($1, $2, $3, 'org_admin', $4)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&org_id)
    .bind(&principal.user_id)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let space_id = create_org_space(&state.pool, &org_id, &created_at)
        .await
        .map_err(ApiError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": org_id,
            "name": payload.name,
            "slug": payload.slug,
            "spaceId": space_id,
        })),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AddOrgMemberRequest {
    username_or_email: String,
    role: String,
}

async fn add_org_member_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org_id): Path<String>,
    Json(payload): Json<AddOrgMemberRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;
    if !is_org_admin(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
    {
        return Err(ApiError::forbidden("only org admins may add members"));
    }
    if payload.role != "org_admin" && payload.role != "org_member" {
        return Err(ApiError::bad_request("role must be org_admin or org_member"));
    }

    let target = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = $1")
        .bind(&payload.username_or_email)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("user not found"))?;

    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM organization_members WHERE org_id = $1 AND user_id = $2",
    )
    .bind(&org_id)
    .bind(&target)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;
    if existing > 0 {
        return Err(ApiError::bad_request("user is already a member"));
    }

    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    sqlx::query(
        "INSERT INTO organization_members (id, org_id, user_id, role, created_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&org_id)
    .bind(&target)
    .bind(&payload.role)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "orgId": org_id, "userId": target, "role": payload.role })),
    ))
}

async fn list_org_members_handler(
    State(state): State<AppState>,
    Path(org_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    if org_member_role(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
        .is_none()
    {
        return Err(ApiError::forbidden("not an organization member"));
    }
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT om.user_id, u.username, om.role \
         FROM organization_members om \
         JOIN users u ON u.id = om.user_id \
         WHERE om.org_id = $1 \
         ORDER BY om.created_at ASC",
    )
    .bind(&org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::from)?;
    let members: Vec<Value> = rows
        .into_iter()
        .map(|(user_id, username, role)| {
            json!({ "userId": user_id, "username": username, "role": role })
        })
        .collect();
    Ok(Json(json!({ "members": members })))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SetOrgMemberRoleRequest {
    role: String,
}

async fn set_org_member_role_handler(
    State(state): State<AppState>,
    Path((org_id, user_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<SetOrgMemberRoleRequest>,
) -> Result<Json<Value>, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;
    if payload.role != "org_admin" && payload.role != "org_member" {
        return Err(ApiError::bad_request("invalid role"));
    }
    if !is_org_admin(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
    {
        return Err(ApiError::forbidden("not an organization admin"));
    }
    let result = sqlx::query(
        "UPDATE organization_members SET role = $1 WHERE org_id = $2 AND user_id = $3",
    )
    .bind(&payload.role)
    .bind(&org_id)
    .bind(&user_id)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;
    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("membership not found"));
    }
    Ok(Json(json!({
        "orgId": org_id,
        "userId": user_id,
        "role": payload.role,
    })))
}

async fn remove_org_member_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((org_id, user_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;
    if !is_org_admin(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
    {
        return Err(ApiError::forbidden("only org admins may remove members"));
    }
    sqlx::query("DELETE FROM organization_members WHERE org_id = $1 AND user_id = $2")
        .bind(&org_id)
        .bind(&user_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}
