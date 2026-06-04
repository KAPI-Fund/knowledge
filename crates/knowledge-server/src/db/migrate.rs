use sqlx::migrate::Migrator;
use sqlx::SqlitePool;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn run(pool: &SqlitePool) -> anyhow::Result<()> {
  MIGRATOR.run(pool).await.map_err(Into::into)
}
