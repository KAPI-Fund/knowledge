pub mod app;
pub mod auth;
pub mod cache;
pub mod chat;
pub mod config;
pub mod db;
pub mod deep_research;
pub mod http;
pub mod multimodal;
pub mod projects;
pub mod providers;
pub mod query;
pub mod retrieval;
pub mod settings;
pub mod tasks;
pub mod users;
pub mod web_search;

use app::state::AppState;
use cache::CacheStore;
use config::AppConfig;
use sqlx::PgPool;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

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
    seed_runtime_settings(&pool, config).await?;
    seed_admin_user(&pool, config).await?;
    let cache = CacheStore::connect(&config.redis_url).await?;
    let state = AppState {
        pool,
        cache,
        project_root: config.project_root.clone(),
        session_ttl_hours: config.session_ttl_hours,
    };
    tasks::recovery::recover_tasks(&state).await?;
    tasks::scheduler::spawn_scheduler(state.clone());
    projects::source_watch::spawn_source_watch_scheduler(state.clone());
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

async fn seed_runtime_settings(pool: &PgPool, config: &AppConfig) -> anyhow::Result<()> {
    let Some(provider_base_url) = config.provider_base_url.clone() else {
        return Ok(());
    };
    let Some(provider_model) = config.provider_model.clone() else {
        return Ok(());
    };

    let current = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>, Option<i64>)>(
    "SELECT provider_mode, provider_base_url, provider_api_key, provider_model, provider_timeout_seconds
     FROM system_settings
     WHERE id = 1",
  )
  .fetch_optional(pool)
  .await?;

    let Some((
        provider_mode,
        current_base_url,
        current_api_key,
        current_model,
        current_timeout_seconds,
    )) = current
    else {
        return Ok(());
    };

    if provider_mode != "deterministic"
        && current_base_url
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        && current_model
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        return Ok(());
    }

    let timeout_seconds = config.provider_timeout_seconds.or(current_timeout_seconds);

    sqlx::query(
        "UPDATE system_settings
     SET provider_mode = $1,
         provider_base_url = $2,
         provider_api_key = $3,
         provider_model = $4,
         provider_timeout_seconds = $5
     WHERE id = 1",
    )
    .bind(
        config
            .provider_mode
            .clone()
            .unwrap_or_else(|| "openai-compatible".to_string()),
    )
    .bind(provider_base_url)
    .bind(config.provider_api_key.clone().or(current_api_key))
    .bind(provider_model)
    .bind(timeout_seconds)
    .execute(pool)
    .await?;

    Ok(())
}

async fn seed_admin_user(pool: &PgPool, config: &AppConfig) -> anyhow::Result<()> {
    let admin_exists =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = 'admin'")
            .fetch_one(pool)
            .await?;
    if admin_exists > 0 {
        return Ok(());
    }

    let Some(ref bootstrap_password) = config.admin_password else {
        tracing::warn!(
            "no admin user exists and KNOWLEDGE_ADMIN_PASSWORD is not set — \
             set it to create a first admin on startup"
        );
        return Ok(());
    };

    let password_hash = crate::auth::password::hash_password(bootstrap_password)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::from("1970-01-01T00:00:00Z"));
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, created_at)
     VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind("admin")
    .bind(password_hash)
    .bind("admin")
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}
