use sqlx::PgPool;

use crate::cache::CacheStore;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cache: CacheStore,
    pub project_root: String,
    pub session_ttl_hours: u64,
    // Mirrors AppConfig::allow_private_fetch: when true the fetch node bypasses
    // its SSRF IP screening (trusted deployments behind a fake-IP proxy).
    pub allow_private_fetch: bool,
}
