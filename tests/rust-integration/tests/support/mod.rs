#[allow(dead_code)]
pub mod mock_openai;

use anyhow::{Context, Result};
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::app::state::AppState;
use knowledge_server::build_app;
use knowledge_server::cache::CacheStore;
use knowledge_server::config::AppConfig;
use redis::AsyncCommands;
use serde_json::{Value, json};
use sqlx::Connection;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::timeout;
use tower::util::ServiceExt;
use uuid::Uuid;

const DEFAULT_POSTGRES_ADMIN_URL: &str =
    "postgres://postgres:postgres@127.0.0.1:55432/postgres?sslmode=disable";
const DEFAULT_POSTGRES_APP_URL_TEMPLATE: &str =
    "postgres://postgres:postgres@127.0.0.1:55432/{database}?sslmode=disable";
const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:56379/";

pub struct TestEnvironment {
    pub database_url: String,
    pub redis_url: String,
    admin_database_url: String,
    database_name: String,
}

impl TestEnvironment {
    pub async fn start(test_name: &str) -> Result<Self> {
        let admin_database_url = std::env::var("KNOWLEDGE_TEST_POSTGRES_ADMIN_URL")
            .unwrap_or_else(|_| DEFAULT_POSTGRES_ADMIN_URL.to_string());
        let app_url_template = std::env::var("KNOWLEDGE_TEST_POSTGRES_APP_URL_TEMPLATE")
            .unwrap_or_else(|_| DEFAULT_POSTGRES_APP_URL_TEMPLATE.to_string());
        let redis_url = std::env::var("KNOWLEDGE_TEST_REDIS_URL")
            .unwrap_or_else(|_| DEFAULT_REDIS_URL.to_string());
        let database_name = format!(
            "knowledge_{}_{}",
            sanitize_name(test_name),
            Uuid::new_v4().simple()
        );

        let mut postgres = timeout(
            Duration::from_secs(5),
            sqlx::PgConnection::connect(&admin_database_url),
        )
        .await
        .with_context(|| {
            format!("timed out connecting to postgres admin url {admin_database_url}")
        })?
        .with_context(|| format!("failed to connect to postgres admin url {admin_database_url}"))?;
        timeout(
            Duration::from_secs(5),
            sqlx::query(&format!("CREATE DATABASE {database_name}")).execute(&mut postgres),
        )
        .await
        .with_context(|| format!("timed out creating database {database_name}"))?
        .with_context(|| format!("failed to create database {database_name}"))?;

        let redis = redis::Client::open(redis_url.clone())?;
        let mut redis_connection = timeout(
            Duration::from_secs(5),
            redis.get_multiplexed_tokio_connection(),
        )
        .await
        .with_context(|| format!("timed out connecting to redis url {redis_url}"))??;
        let _: String = redis::cmd("PING")
            .query_async(&mut redis_connection)
            .await
            .with_context(|| format!("failed to connect to redis url {redis_url}"))?;

        Ok(Self {
            database_url: app_url_template.replace("{database}", &database_name),
            redis_url,
            admin_database_url,
            database_name,
        })
    }

    #[allow(dead_code)]
    pub async fn redis_get(&self, key: &str) -> Result<Option<String>> {
        let client = redis::Client::open(self.redis_url.clone())?;
        let mut connection = client.get_multiplexed_tokio_connection().await?;
        let value: Option<String> = connection.get(key).await?;
        Ok(value)
    }
}

#[allow(dead_code)]
pub async fn bootstrap_state_without_scheduler(config: &AppConfig) -> Result<AppState> {
    let pool = knowledge_server::db::pool::connect_pool(&config.database_url).await?;
    knowledge_server::db::migrate::run(&pool).await?;
    knowledge_server::seed_admin_user(&pool, config).await?;
    let cache = CacheStore::connect(&config.redis_url).await?;

    Ok(AppState {
        pool,
        cache,
        project_root: config.project_root.clone(),
        session_ttl_hours: config.session_ttl_hours,
        allow_private_fetch: config.allow_private_fetch,
    })
}

/// Seed an active LLM connection for chat/analyze. Mirrors what the admin UI does
/// (create_connection makes the first connection active), replacing the old
/// legacy flat `provider_*` column writes that were dropped in migration 0016.
#[allow(dead_code)]
pub async fn seed_provider_connection(
    pool: &sqlx::PgPool,
    base_url: &str,
    api_key: &str,
    model: &str,
    timeout_seconds: i64,
) {
    knowledge_server::providers::create_connection(
        pool,
        &knowledge_server::providers::NewConnection {
            label: "Test".to_string(),
            base_url: base_url.to_string(),
            api_key: Some(api_key.to_string()),
            model: model.to_string(),
            timeout_seconds: Some(timeout_seconds),
        },
    )
    .await
    .expect("failed to seed provider connection");
}

/// Seed the independent embedding block on the singleton settings row, replacing
/// the old `provider_embedding_model` legacy column write.
#[allow(dead_code)]
pub async fn seed_embedding(
    pool: &sqlx::PgPool,
    base_url: &str,
    api_key: &str,
    model: &str,
    timeout_seconds: i64,
) {
    sqlx::query(
        "UPDATE system_settings
         SET embedding_enabled = true,
             embedding_base_url = $1,
             embedding_api_key = $2,
             embedding_model = $3,
             embedding_timeout_seconds = $4
         WHERE id = 1",
    )
    .bind(base_url)
    .bind(api_key)
    .bind(model)
    .bind(timeout_seconds)
    .execute(pool)
    .await
    .expect("failed to seed embedding config");
}

#[allow(dead_code)]
pub fn bind_project_root_alias(alias: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = alias.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create parent directory for '{}'",
                alias.display()
            )
        })?;
    }

    #[cfg(windows)]
    {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(alias.as_os_str())
            .arg(target.as_os_str())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .with_context(|| {
                format!(
                    "failed to create junction from '{}' to '{}'",
                    alias.display(),
                    target.display()
                )
            })?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "failed to create junction from '{}' to '{}' (target exists: {})",
                alias.display(),
                target.display(),
                target.exists()
            ));
        }
    }

    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(target, alias).with_context(|| {
            format!(
                "failed to create symlink from '{}' to '{}'",
                alias.display(),
                target.display()
            )
        })?;
    }

    Ok(())
}

#[allow(dead_code)]
pub fn normalized_project_path(path: &str) -> PathBuf {
    let value = if let Some(stripped) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", stripped)
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(path).to_string()
    };

    PathBuf::from(value)
}

#[allow(dead_code)]
pub async fn create_project_with_alias(
    state: AppState,
    cookie: &str,
    csrf: &str,
    project_root: PathBuf,
) -> String {
    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/projects")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", csrf)
                .body(Body::from(
                    json!({
                      "name": project_root.file_name().unwrap().to_string_lossy()
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let payload = read_json(response.into_body()).await;
    let root_path = payload.get("rootPath").and_then(Value::as_str).unwrap();
    let actual_root = normalized_project_path(root_path);
    bind_project_root_alias(&project_root, &actual_root).unwrap();
    payload
        .get("id")
        .and_then(Value::as_str)
        .unwrap()
        .to_string()
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        let admin_database_url = self.admin_database_url.clone();
        let database_name = self.database_name.clone();
        let _ = std::thread::spawn(move || {
      let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();

      if let Ok(runtime) = runtime {
        runtime.block_on(async move {
          if let Ok(mut postgres) = sqlx::PgConnection::connect(&admin_database_url).await {
            let _ = sqlx::query(
              "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1 AND pid <> pg_backend_pid()",
            )
            .bind(&database_name)
            .execute(&mut postgres)
            .await;
            let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {database_name}"))
              .execute(&mut postgres)
              .await;
          }
        });
      }
    })
    .join();
    }
}

fn sanitize_name(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}
