use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, Clone)]
pub struct AppConfig {
  pub bind_addr: SocketAddr,
  pub database_url: String,
  pub session_ttl_hours: u64,
}

impl AppConfig {
  pub fn for_tests(database_url: String) -> Self {
    Self {
      bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
      database_url,
      session_ttl_hours: 12,
    }
  }
}
