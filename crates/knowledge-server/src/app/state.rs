use sqlx::PgPool;

use crate::cache::CacheStore;

#[derive(Clone)]
pub struct AppState {
  pub pool: PgPool,
  pub cache: CacheStore,
  pub session_ttl_hours: u64,
}
