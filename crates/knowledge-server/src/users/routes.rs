use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use crate::app::state::AppState;

pub fn router() -> Router<AppState> {
  Router::new().route("/api/users", get(list_users))
}

async fn list_users() -> Json<serde_json::Value> {
  Json(json!({ "users": [] }))
}
