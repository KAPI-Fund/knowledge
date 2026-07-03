use sqlx::migrate::Migrator;
use sqlx::PgPool;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn run(pool: &PgPool) -> anyhow::Result<()> {
  MIGRATOR.run(pool).await.map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::MIGRATOR;

    #[test]
    fn migrator_embeds_multi_provider_migration() {
        // 0015 must be compiled into the embedded migration set.
        assert!(
            MIGRATOR.iter().any(|m| m.version == 15),
            "migration 0015 not embedded"
        );
    }
}
