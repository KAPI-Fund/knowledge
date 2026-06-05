use sqlx::migrate::Migrator;
use sqlx::PgPool;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn run(pool: &PgPool) -> anyhow::Result<()> {
  MIGRATOR.run(pool).await.map_err(Into::into)
}
