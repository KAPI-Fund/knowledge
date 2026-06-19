use axum::Json;
use axum::Router;
use axum::extract::State;
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
use crate::tenancy::slug::validate_slug;
use crate::tenancy::spaces::create_org_space;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/orgs", post(create_org_handler))
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
