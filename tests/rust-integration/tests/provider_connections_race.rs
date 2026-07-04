mod support;

use anyhow::Result;
use knowledge_server::providers::{create_connection, list_connections, NewConnection};
use support::{bootstrap_state_without_scheduler, TestEnvironment};
use tempfile::tempdir;

use knowledge_server::config::AppConfig;

/// Concurrently creating the first connections must still leave EXACTLY ONE row
/// active. Before the fix, `create_connection` used a non-transactional
/// COUNT -> is_active decision, so several racers could each observe an empty
/// table and all insert is_active=true, breaking the single-active invariant.
#[tokio::test]
async fn concurrent_create_connection_keeps_exactly_one_active() -> Result<()> {
    let env = TestEnvironment::start("provider-connections-race").await?;
    let project_root = tempdir()?;
    let config = AppConfig {
        bind_addr: "127.0.0.1:0".parse()?,
        database_url: env.database_url.clone(),
        redis_url: env.redis_url.clone(),
        project_root: project_root.path().to_string_lossy().to_string(),
        session_ttl_hours: 12,
        admin_password: Some("secret-password".to_string()),
    };
    let state = bootstrap_state_without_scheduler(&config).await?;

    // Fire many creates at once against a fresh (empty) provider_connections table.
    let mut handles = Vec::new();
    for i in 0..16 {
        let pool = state.pool.clone();
        handles.push(tokio::spawn(async move {
            create_connection(
                &pool,
                &NewConnection {
                    label: format!("conn-{i}"),
                    base_url: "https://api.example.com/v1".to_string(),
                    api_key: Some("sk-test".to_string()),
                    model: "gpt-4o".to_string(),
                    timeout_seconds: Some(30),
                },
            )
            .await
        }));
    }

    for handle in handles {
        handle.await.expect("create task panicked")?;
    }

    let rows = list_connections(&state.pool).await?;
    assert_eq!(rows.len(), 16, "all creates should have inserted a row");
    let active_count = rows.iter().filter(|c| c.is_active).count();
    assert_eq!(active_count, 1, "exactly one connection must be active after a concurrent race");

    // sort_order values must also be unique (no two racers picked the same slot).
    let mut sort_orders: Vec<i32> = rows.iter().map(|c| c.sort_order).collect();
    sort_orders.sort_unstable();
    sort_orders.dedup();
    assert_eq!(sort_orders.len(), 16, "sort_order values must be unique");

    Ok(())
}
