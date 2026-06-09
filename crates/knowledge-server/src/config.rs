use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, Clone)]
pub struct AppConfig {
  pub bind_addr: SocketAddr,
  pub database_url: String,
  pub redis_url: String,
  pub project_root: String,
  pub provider_mode: Option<String>,
  pub provider_base_url: Option<String>,
  pub provider_api_key: Option<String>,
  pub provider_model: Option<String>,
  pub provider_timeout_seconds: Option<i64>,
  pub session_ttl_hours: u64,
}

impl AppConfig {
  pub fn for_tests(database_url: String, redis_url: String) -> Self {
    Self {
      bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
      database_url,
      redis_url,
      project_root: std::env::temp_dir()
        .join("knowledge-projects")
        .to_string_lossy()
        .to_string(),
      provider_mode: None,
      provider_base_url: None,
      provider_api_key: None,
      provider_model: None,
      provider_timeout_seconds: None,
      session_ttl_hours: 12,
    }
  }

  pub fn from_env() -> Self {
    let bind_addr = std::env::var("KNOWLEDGE_BIND_ADDR")
      .ok()
      .and_then(|value| value.parse().ok())
      .unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4001));

    let database_url = std::env::var("KNOWLEDGE_DATABASE_URL")
      .unwrap_or_else(|_| {
        "postgres://postgres:postgres@127.0.0.1:55432/knowledge?sslmode=disable".to_string()
      });
    let redis_url = std::env::var("KNOWLEDGE_REDIS_URL")
      .unwrap_or_else(|_| "redis://127.0.0.1:56379/".to_string());
    let project_root = std::env::var("KNOWLEDGE_PROJECT_ROOT").unwrap_or_else(|_| {
      std::env::current_dir()
        .unwrap_or_default()
        .join(".e2e")
        .to_string_lossy()
        .to_string()
    });
    let provider_mode = std::env::var("KNOWLEDGE_PROVIDER_MODE").ok();
    let provider_base_url = std::env::var("KNOWLEDGE_PROVIDER_BASE_URL").ok();
    let provider_api_key = std::env::var("KNOWLEDGE_PROVIDER_API_KEY").ok();
    let provider_model = std::env::var("KNOWLEDGE_PROVIDER_MODEL").ok();
    let provider_timeout_seconds = std::env::var("KNOWLEDGE_PROVIDER_TIMEOUT_SECONDS")
      .ok()
      .and_then(|value| value.parse::<i64>().ok());

    Self {
      bind_addr,
      database_url,
      redis_url,
      project_root,
      provider_mode,
      provider_base_url,
      provider_api_key,
      provider_model,
      provider_timeout_seconds,
      session_ttl_hours: 12,
    }
  }
}
