use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

pub async fn connect_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
  let options = SqliteConnectOptions::from_str(database_url)?
    .create_if_missing(true)
    .foreign_keys(true);

  SqlitePoolOptions::new()
    .max_connections(1)
    .connect_with(options)
    .await
    .map_err(Into::into)
}
