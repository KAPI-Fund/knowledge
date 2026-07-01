use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::app::state::AppState;
use crate::assets::store;
use crate::auth::principal::resolve_principal;
use crate::http::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/assets/{id}", get(get_asset_handler))
}

async fn get_asset_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    let asset = store::get_asset(&state.pool, &id, &principal.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("asset not found"))?;
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, asset.mime)
        .header(header::CACHE_CONTROL, "private, max-age=31536000, immutable")
        .body(Body::from(asset.bytes))
        .map_err(|_| ApiError::internal("failed to build asset response"))?;
    Ok(response)
}
