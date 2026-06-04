use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;

use crate::auth;
use crate::app::state::AppState;
use crate::projects;
use crate::users;

pub fn build_router(state: AppState) -> Router {
  Router::new()
    .route("/api/health", get(health))
    .merge(auth::routes::router())
    .merge(projects::routes::router())
    .merge(users::routes::router())
    .with_state(state)
}

async fn health() -> StatusCode {
  StatusCode::OK
}
