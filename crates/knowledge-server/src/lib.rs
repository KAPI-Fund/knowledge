pub mod app;
pub mod auth;
pub mod cache;
pub mod config;
pub mod db;
pub mod http;
pub mod providers;
pub mod projects;
pub mod retrieval;
pub mod settings;
pub mod tasks;
pub mod users;

use app::state::AppState;
use cache::CacheStore;
use config::AppConfig;
use sqlx::PgPool;

pub async fn run() -> anyhow::Result<()> {
  let config = AppConfig::from_env();
  let state = bootstrap_state(&config).await?;
  let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
  axum::serve(listener, build_app(state)).await?;
  Ok(())
}

pub async fn bootstrap_state(config: &AppConfig) -> anyhow::Result<AppState> {
  let pool = db::pool::connect_pool(&config.database_url).await?;
  db::migrate::run(&pool).await?;
  let cache = CacheStore::connect(&config.redis_url).await?;
  let state = AppState {
    pool,
    cache,
    session_ttl_hours: config.session_ttl_hours,
  };
  tasks::recovery::recover_tasks(&state).await?;
  tasks::scheduler::spawn_scheduler(state.clone());
  Ok(state)
}

pub fn build_app(state: AppState) -> axum::Router {
  http::router::build_router(state)
}

pub async fn table_exists(pool: &PgPool, table_name: &str) -> anyhow::Result<bool> {
  let exists = sqlx::query_scalar::<_, Option<String>>("SELECT to_regclass($1)::text")
    .bind(format!("public.{table_name}"))
    .fetch_one(pool)
  .await?;

  Ok(exists.is_some())
}
