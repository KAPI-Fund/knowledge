mod support;

use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, table_exists};
use support::TestEnvironment;
use uuid::Uuid;

#[tokio::test]
async fn migration_creates_tenancy_tables_and_personal_space_uniqueness() {
  let env = TestEnvironment::start("tenancy-migration").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();

  assert!(table_exists(&state.pool, "organizations").await.unwrap());
  assert!(table_exists(&state.pool, "organization_members").await.unwrap());
  assert!(table_exists(&state.pool, "spaces").await.unwrap());

  // The seeded admin has exactly one personal space.
  let admin_id = sqlx::query_scalar::<_, String>(
    "SELECT id FROM users WHERE username = 'admin'",
  )
  .fetch_one(&state.pool)
  .await
  .unwrap();

  let space_count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM spaces WHERE kind = 'personal' AND owner_user_id = $1",
  )
  .bind(&admin_id)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(space_count, 1, "admin must have exactly one personal space");

  // The partial unique index forbids a second personal space for the same user.
  let duplicate = sqlx::query(
    "INSERT INTO spaces (id, kind, owner_user_id, org_id, created_at) \
     VALUES ($1, 'personal', $2, NULL, '2026-01-01T00:00:00Z')",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(&admin_id)
  .execute(&state.pool)
  .await;
  assert!(duplicate.is_err(), "second personal space for a user must be rejected");
}
