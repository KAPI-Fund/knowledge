pub mod app;
pub mod auth;
pub mod config;
pub mod db;
pub mod http;
pub mod projects;
pub mod users;

use app::state::AppState;
use config::AppConfig;
use sqlx::SqlitePool;

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
  Ok(AppState { pool })
}

pub fn build_app(state: AppState) -> axum::Router {
  http::router::build_router(state)
}

pub async fn table_exists(pool: &SqlitePool, table_name: &str) -> anyhow::Result<bool> {
  let row: Option<(String,)> = sqlx::query_as(
    "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1",
  )
  .bind(table_name)
  .fetch_optional(pool)
  .await?;

  Ok(row.is_some())
}
