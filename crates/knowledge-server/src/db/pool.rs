use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn connect_pool(database_url: &str) -> anyhow::Result<PgPool> {
  PgPoolOptions::new()
    .max_connections(5)
    .connect(database_url)
    .await
    .map_err(Into::into)
}
