use axum::Json;
use axum::routing::get;
use axum::Router;
use serde_json::json;

use crate::auth;
use crate::app::state::AppState;
use crate::canvas;
use crate::chat;
use crate::deep_research;
use crate::projects;
use crate::settings;
use crate::tenancy;
use crate::users;
use crate::web_search;

pub fn build_router(state: AppState) -> Router {
  Router::new()
    .route("/api/health", get(health))
    .merge(auth::routes::router())
    .merge(canvas::routes::router())
    .merge(chat::routes::router())
    .merge(deep_research::routes::router())
    .merge(projects::routes::router())
    .merge(settings::routes::router())
    .merge(tenancy::grants::router())
    .merge(tenancy::orgs::router())
    .merge(tenancy::spaces_api::router())
    .merge(tenancy::teams::router())
    .merge(users::routes::router())
    .merge(web_search::routes::router())
    .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
  Json(json!({ "ok": true, "service": "knowledge-server" }))
}
