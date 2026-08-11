use axum::extract::State;
use axum::Json;
use axum::routing::get;
use axum::Router;
use serde_json::json;

use crate::agent;
use crate::assets;
use crate::auth;
use crate::app::state::AppState;
use crate::canvas;
use crate::chat;
use crate::deep_research;
use crate::mcp;
use crate::projects;
use crate::settings;
use crate::skills;
use crate::tenancy;
use crate::users;
use crate::web_search;

pub fn build_router(state: AppState) -> Router {
  Router::new()
    .route("/api/health", get(health))
    .route(
      "/api/mcp",
      axum::routing::post(mcp::routes::handle_mcp).get(mcp::routes::method_not_allowed),
    )
    .merge(agent::routes::router())
    .merge(auth::routes::router())
    .merge(assets::routes::router())
    .merge(canvas::routes::router())
    .merge(chat::routes::router())
    .merge(deep_research::routes::router())
    .merge(projects::routes::router())
    .merge(settings::routes::router())
    .merge(skills::routes::router())
    .merge(tenancy::grants::router())
    .merge(tenancy::orgs::router())
    .merge(tenancy::spaces_api::router())
    .merge(tenancy::teams::router())
    .merge(users::routes::router())
    .merge(web_search::routes::router())
    .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
  let mcp_enabled = sqlx::query_scalar::<_, bool>(
    "SELECT mcp_enabled FROM system_settings WHERE id = 1",
  )
  .fetch_optional(&state.pool)
  .await
  .ok()
  .flatten()
  .unwrap_or(false);
  Json(json!({ "ok": true, "service": "knowledge-server", "mcpEnabled": mcp_enabled }))
}
