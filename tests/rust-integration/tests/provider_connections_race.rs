mod support;

use anyhow::Result;
use knowledge_server::providers::{
    activate_connection, create_connection, delete_connection, list_connections, NewConnection,
};
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
        allow_private_fetch: false,
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

/// Concurrently activating and deleting connections must never leave the table
/// with zero active rows while survivors remain. Before the fix, delete read
/// `was_active` outside any shared lock and skipped promotion when it saw the row
/// as inactive; a concurrent activate could flip that row active in between,
/// leaving 0 active after the delete. All three active-state mutations now contend
/// on one advisory lock, and delete unconditionally re-promotes when nothing is
/// active.
#[tokio::test]
async fn concurrent_activate_and_delete_never_drop_to_zero_active() -> Result<()> {
    let env = TestEnvironment::start("provider-connections-activate-delete-race").await?;
    let project_root = tempdir()?;
    let config = AppConfig {
        bind_addr: "127.0.0.1:0".parse()?,
        database_url: env.database_url.clone(),
        redis_url: env.redis_url.clone(),
        project_root: project_root.path().to_string_lossy().to_string(),
        session_ttl_hours: 12,
        admin_password: Some("secret-password".to_string()),
        allow_private_fetch: false,
    };
    let state = bootstrap_state_without_scheduler(&config).await?;

    // Seed a handful of connections up front (serially, so ids are known).
    let mut ids = Vec::new();
    for i in 0..8 {
        let created = create_connection(
            &state.pool,
            &NewConnection {
                label: format!("conn-{i}"),
                base_url: "https://api.example.com/v1".to_string(),
                api_key: Some("sk-test".to_string()),
                model: "gpt-4o".to_string(),
                timeout_seconds: Some(30),
            },
        )
        .await?;
        ids.push(created.id);
    }

    // Concurrently: activate every connection, and delete half of them. Whatever
    // the interleaving, the single-active invariant must hold for the survivors.
    let mut handles = Vec::new();
    for id in ids.iter().cloned() {
        let pool = state.pool.clone();
        handles.push(tokio::spawn(
            async move { activate_connection(&pool, &id).await },
        ));
    }
    for id in ids.iter().take(4).cloned() {
        let pool = state.pool.clone();
        handles.push(tokio::spawn(
            async move { delete_connection(&pool, &id).await },
        ));
    }
    for handle in handles {
        // Individual ops may legitimately fail (e.g. activate races a delete of the
        // same id -> not_found); only unexpected panics should abort the test.
        let _ = handle.await.expect("task panicked");
    }

    let rows = list_connections(&state.pool).await?;
    assert!(!rows.is_empty(), "the 4 undeleted connections must survive");
    let active_count = rows.iter().filter(|c| c.is_active).count();
    assert_eq!(
        active_count, 1,
        "exactly one connection must be active after concurrent activate/delete (never 0)"
    );

    Ok(())
}
