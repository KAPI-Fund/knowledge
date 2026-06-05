use anyhow::{Context, Result};
use redis::AsyncCommands;
use sqlx::Connection;
use std::time::Duration;
use tokio::time::timeout;
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
    let redis_url =
      std::env::var("KNOWLEDGE_TEST_REDIS_URL").unwrap_or_else(|_| DEFAULT_REDIS_URL.to_string());
    let database_name = format!("knowledge_{}_{}", sanitize_name(test_name), Uuid::new_v4().simple());

    let mut postgres = timeout(
      Duration::from_secs(5),
      sqlx::PgConnection::connect(&admin_database_url),
    )
    .await
    .with_context(|| format!("timed out connecting to postgres admin url {admin_database_url}"))?
    .with_context(|| format!("failed to connect to postgres admin url {admin_database_url}"))?;
    timeout(
      Duration::from_secs(5),
      sqlx::query(&format!("CREATE DATABASE {database_name}")).execute(&mut postgres),
    )
    .await
    .with_context(|| format!("timed out creating database {database_name}"))?
    .with_context(|| format!("failed to create database {database_name}"))?;

    let redis = redis::Client::open(redis_url.clone())?;
    let mut redis_connection = timeout(Duration::from_secs(5), redis.get_multiplexed_tokio_connection())
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
