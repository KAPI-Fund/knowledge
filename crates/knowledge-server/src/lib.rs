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
pub mod skills;
pub mod tasks;
pub mod tenancy;
pub mod users;
pub mod web_fetch;
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
    seed_admin_user(&pool, config).await?;
    let cache = CacheStore::connect(&config.redis_url).await?;
    let skills_dir = std::env::var("KNOWLEDGE_SKILLS_DIR")
        .unwrap_or_else(|_| "/app/skills".to_string());
    let descriptors =
        skills::SkillRegistry::load_from_dir(std::path::Path::new(&skills_dir))?;
    let skill_registry = skills::SkillRegistry::new(descriptors);
    let executor: std::sync::Arc<dyn crate::canvas::executor::SkillExecutor> =
        std::sync::Arc::new(crate::canvas::executor::CubeExecutor::new(
            config.skill_runner_url.clone(),
        ));
    let state = AppState {
        pool,
        cache,
        project_root: config.project_root.clone(),
        session_ttl_hours: config.session_ttl_hours,
        skill_registry,
        executor,
    };
    tasks::recovery::recover_tasks(&state).await?;
    tasks::scheduler::spawn_scheduler(state.clone());
    crate::canvas::skill_jobs::recover_skill_jobs(&state.pool).await?;
    crate::canvas::skill_worker::spawn_skill_worker(state.clone());
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
