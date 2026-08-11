mod support;

use anyhow::Result;
use knowledge_server::bootstrap_state;
use knowledge_server::config::AppConfig;
use support::TestEnvironment;
use tempfile::tempdir;

#[tokio::test]
async fn bootstrap_seeds_admin_user_and_leaves_provider_unconfigured() -> Result<()> {
  let env = TestEnvironment::start("docker-stack-smoke").await?;
  let project_root = tempdir()?;
  let config = AppConfig {
    bind_addr: "127.0.0.1:0".parse()?,
    database_url: env.database_url.clone(),
    redis_url: env.redis_url.clone(),
    project_root: project_root.path().to_string_lossy().to_string(),
    session_ttl_hours: 12,
    admin_password: Some("secret-password".to_string()),
  };

  let state = bootstrap_state(&config).await?;

  // Config is UI-only now (no env seeding): a fresh install must start with zero
  // provider connections, so nothing is auto-restored on boot.
  let connection_count =
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM provider_connections")
      .fetch_one(&state.pool)
      .await?;
  assert_eq!(connection_count, 0);

  let admin_count =
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = 'admin'")
      .fetch_one(&state.pool)
      .await?;
  assert_eq!(admin_count, 1);

  Ok(())
}
