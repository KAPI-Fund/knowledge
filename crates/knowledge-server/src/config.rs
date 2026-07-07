use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, Clone)]
pub struct AppConfig {
  pub bind_addr: SocketAddr,
  pub database_url: String,
  pub redis_url: String,
  pub project_root: String,
  pub session_ttl_hours: u64,
  pub admin_password: Option<String>,
  pub skill_runner_url: String,
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
      session_ttl_hours: 12,
      admin_password: Some("secret-password".to_string()),
      skill_runner_url: "http://127.0.0.1:4600".to_string(),
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
    let admin_password = std::env::var("KNOWLEDGE_ADMIN_PASSWORD").ok();
    let skill_runner_url = std::env::var("KNOWLEDGE_SKILL_RUNNER_URL")
      .unwrap_or_else(|_| "http://127.0.0.1:4600".to_string());

    Self {
      bind_addr,
      database_url,
      redis_url,
      project_root,
      session_ttl_hours: 12,
      admin_password,
      skill_runner_url,
    }
  }
}
