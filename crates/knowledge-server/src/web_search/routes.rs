use axum::Router;

use crate::app::state::AppState;

pub fn router() -> Router<AppState> {
  Router::new()
}
