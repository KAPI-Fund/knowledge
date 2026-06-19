use sqlx::PgPool;
use uuid::Uuid;

/// Return the id of the user's personal space, creating it if absent.
///
/// The `spaces_personal_owner` partial unique index guarantees at most one
/// personal space per user; this function is the single writer.
pub async fn ensure_personal_space(
    pool: &PgPool,
    user_id: &str,
    created_at: &str,
) -> Result<String, sqlx::Error> {
    if let Some(id) = personal_space_id(pool, user_id).await? {
        return Ok(id);
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO spaces (id, kind, owner_user_id, org_id, created_at) \
         VALUES ($1, 'personal', $2, NULL, $3)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Look up the user's personal space id, if one exists.
pub async fn personal_space_id(
    pool: &PgPool,
    user_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT id FROM spaces WHERE kind = 'personal' AND owner_user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}
