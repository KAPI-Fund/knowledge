mod support;

use anyhow::Result;
use knowledge_server::bootstrap_state;
use knowledge_server::config::AppConfig;
use support::TestEnvironment;
use tempfile::tempdir;

#[tokio::test]
async fn bootstrap_seeds_provider_defaults_and_admin_user_when_env_is_present() -> Result<()> {
  let env = TestEnvironment::start("docker-stack-smoke").await?;
  let project_root = tempdir()?;
  let config = AppConfig {
    bind_addr: "127.0.0.1:0".parse()?,
    database_url: env.database_url.clone(),
    redis_url: env.redis_url.clone(),
    project_root: project_root.path().to_string_lossy().to_string(),
    provider_mode: Some("openai-compatible".to_string()),
    provider_base_url: Some("https://backend.intelalloc.com".to_string()),
    provider_api_key: Some("test-key".to_string()),
    provider_model: Some("gpt-5.4".to_string()),
    provider_timeout_seconds: Some(180),
    session_ttl_hours: 12,
    admin_password: Some("secret-password".to_string()),
  };

  let state = bootstrap_state(&config).await?;
  let row = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>, Option<i64>)>(
    "SELECT provider_mode, provider_base_url, provider_api_key, provider_model, provider_timeout_seconds
     FROM system_settings
     WHERE id = 1",
  )
  .fetch_one(&state.pool)
  .await?;

  assert_eq!(row.0, "openai-compatible");
  assert_eq!(row.1.as_deref(), Some("https://backend.intelalloc.com"));
  assert_eq!(row.2.as_deref(), Some("test-key"));
  assert_eq!(row.3.as_deref(), Some("gpt-5.4"));
  assert_eq!(row.4, Some(180));

  let admin_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = 'admin'")
    .fetch_one(&state.pool)
    .await?;
  assert_eq!(admin_count, 1);

  Ok(())
}
