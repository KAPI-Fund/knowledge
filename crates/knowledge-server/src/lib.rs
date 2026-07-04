pub mod app;
pub mod assets;
pub mod auth;
pub mod cache;
pub mod canvas;
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
pub mod tenancy;
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
    ensure_seed_provider_connection(&pool).await?;
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

/// Seed the first provider_connections row from the legacy flat columns when the
/// table is empty. Migration 0015 runs before env seeding, so on a fresh DB it
/// finds the legacy columns blank and seeds nothing; this step runs *after*
/// seed_runtime_settings has populated them from the environment, so the active
/// connection the ingest tasks resolve against actually exists.
async fn ensure_seed_provider_connection(pool: &PgPool) -> anyhow::Result<()> {
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_connections")
        .fetch_one(pool)
        .await?;
    if existing > 0 {
        return Ok(());
    }

    let row = sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>, Option<i64>)>(
        "SELECT provider_base_url, provider_api_key, provider_model, provider_timeout_seconds
         FROM system_settings
         WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?;

    let Some((base_url, api_key, model, timeout_seconds)) = row else {
        return Ok(());
    };

    let base_url = base_url.unwrap_or_default();
    let model = model.unwrap_or_default();
    if base_url.trim().is_empty() || model.trim().is_empty() {
        return Ok(());
    }

    let id = Uuid::new_v4().to_string();
    let ts = OffsetDateTime::now_utc().format(&Rfc3339)?;
    sqlx::query(
        "INSERT INTO provider_connections
           (id, label, base_url, api_key, model, timeout_seconds, is_active, sort_order, created_at, updated_at)
         VALUES ($1, 'Default', $2, $3, $4, $5, true, 0, $6, $6)",
    )
    .bind(&id)
    .bind(base_url)
    .bind(api_key)
    .bind(model)
    .bind(timeout_seconds)
    .bind(&ts)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn seed_admin_user(pool: &PgPool, config: &AppConfig) -> anyhow::Result<()> {
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

    let admin_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, created_at)
     VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&admin_id)
    .bind("admin")
    .bind(password_hash)
    .bind("operator")
    .bind(&now)
    .execute(pool)
    .await?;

    crate::tenancy::spaces::ensure_personal_space(pool, &admin_id, &now).await?;

    Ok(())
}
