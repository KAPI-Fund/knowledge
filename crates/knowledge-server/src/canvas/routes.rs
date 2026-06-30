use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::app::state::AppState;
use crate::auth::principal::{resolve_principal, Principal};
use crate::canvas::document::CanvasDocument;
use crate::canvas::store;
use crate::http::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/canvases", get(list_handler).post(create_handler))
        .route(
            "/api/canvases/{id}",
            get(get_handler).put(save_handler).delete(delete_handler),
        )
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateCanvasRequest {
    #[serde(default)]
    pub title: Option<String>,
}

impl CreateCanvasRequest {
    pub fn resolved_title(&self) -> String {
        match &self.title {
            Some(t) => {
                let trimmed = t.trim();
                if trimmed.is_empty() {
                    "Untitled canvas".to_string()
                } else {
                    trimmed.to_string()
                }
            }
            None => "Untitled canvas".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SaveCanvasRequest {
    pub title: String,
    pub document: CanvasDocument,
}

impl SaveCanvasRequest {
    pub fn document_text(&self) -> String {
        serde_json::to_string(&self.document).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Debug, Serialize)]
pub struct CanvasResponse {
    pub id: String,
    pub title: String,
    pub document: CanvasDocument,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// CSRF helper
// ---------------------------------------------------------------------------

fn require_csrf(principal: &Principal, headers: &HeaderMap) -> Result<(), ApiError> {
    if principal.requires_csrf() {
        let expected = principal
            .csrf_token
            .as_deref()
            .ok_or_else(|| ApiError::unauthorized("missing csrf token"))?;
        let supplied = headers
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if supplied.is_empty() || supplied != expected {
            return Err(ApiError::unauthorized("invalid csrf token"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<store::CanvasSummary>>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    let records = store::list_canvases(&state.pool, &principal.user_id).await?;
    let summaries: Vec<store::CanvasSummary> = records.iter().map(store::CanvasSummary::from).collect();
    Ok(Json(summaries))
}

async fn create_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateCanvasRequest>,
) -> Result<Json<CanvasResponse>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;
    let rec = store::create_canvas(&state.pool, &principal.user_id, &body.resolved_title()).await?;
    let document = rec.parse_document().map_err(|_| ApiError::internal("corrupt document"))?;
    Ok(Json(CanvasResponse {
        id: rec.id,
        title: rec.title,
        document,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
    }))
}

async fn get_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<CanvasResponse>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    let rec = store::get_canvas(&state.pool, &id, &principal.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("canvas not found"))?;
    let document = rec.parse_document().map_err(|_| ApiError::internal("corrupt document"))?;
    Ok(Json(CanvasResponse {
        id: rec.id,
        title: rec.title,
        document,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
    }))
}

async fn save_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SaveCanvasRequest>,
) -> Result<Json<CanvasResponse>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;
    let rec = store::update_canvas(
        &state.pool,
        &id,
        &principal.user_id,
        &body.title,
        &body.document_text(),
    )
    .await?
    .ok_or_else(|| ApiError::not_found("canvas not found"))?;
    let document = rec.parse_document().map_err(|_| ApiError::internal("corrupt document"))?;
    Ok(Json(CanvasResponse {
        id: rec.id,
        title: rec.title,
        document,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
    }))
}

async fn delete_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;
    let deleted = store::delete_canvas(&state.pool, &id, &principal.user_id).await?;
    if !deleted {
        return Err(ApiError::not_found("canvas not found"));
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_request_defaults_title_when_absent() {
        let req: CreateCanvasRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(req.resolved_title(), "Untitled canvas");
    }

    #[test]
    fn create_request_uses_provided_title() {
        let req: CreateCanvasRequest =
            serde_json::from_str(r#"{"title":"Research B"}"#).unwrap();
        assert_eq!(req.resolved_title(), "Research B");
    }

    #[test]
    fn save_request_serializes_document_to_text() {
        let req: SaveCanvasRequest = serde_json::from_str(
            r#"{"title":"T","document":{"nodes":[],"edges":[],"viewport":{"x":0,"y":0,"zoom":1}}}"#,
        )
        .unwrap();
        let text = req.document_text();
        assert!(text.contains("\"viewport\""));
        let _doc: crate::canvas::document::CanvasDocument =
            serde_json::from_str(&text).unwrap();
    }
}
