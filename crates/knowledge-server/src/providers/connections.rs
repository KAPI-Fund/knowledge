use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::providers::OpenAiCompatibleProvider;

/// A row of the LLM preset connection list.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProviderConnection {
    pub id: String,
    pub label: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout_seconds: Option<i64>,
    pub is_active: bool,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// The resolved active connection used to build a chat/analyze provider.
#[derive(Debug, Clone)]
pub struct ActiveConnection {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_seconds: i64,
}

impl ActiveConnection {
    /// Build an OpenAI-compatible provider from this connection.
    pub fn provider(&self) -> OpenAiCompatibleProvider {
        OpenAiCompatibleProvider::new(
            self.base_url.clone(),
            self.api_key.clone(),
            self.model.clone(),
            self.timeout_seconds,
        )
    }
}

impl From<&ProviderConnection> for ActiveConnection {
    fn from(c: &ProviderConnection) -> Self {
        Self {
            base_url: c.base_url.clone(),
            api_key: c.api_key.clone().unwrap_or_default(),
            model: c.model.clone(),
            timeout_seconds: c.timeout_seconds.unwrap_or(30),
        }
    }
}

/// Pure: pick the active connection from a fetched list. The service layer keeps
/// exactly one active, but if the invariant is ever violated we deterministically
/// choose the lowest sort_order active row.
pub fn resolve_active(rows: &[ProviderConnection]) -> Option<&ProviderConnection> {
    rows.iter()
        .filter(|c| c.is_active)
        .min_by_key(|c| c.sort_order)
}

const SELECT_COLUMNS: &str = "id, label, base_url, api_key, model, timeout_seconds, is_active, sort_order, created_at, updated_at";

/// Transaction-scoped advisory lock key that serializes every mutation touching
/// the `is_active` invariant — create, activate, and delete. Without a single
/// shared lock these can interleave into a 0- or 2-active state (e.g. a delete
/// that read the row as inactive racing an activate that just made it active).
/// The value is arbitrary but must stay stable so all callers contend on the same
/// lock. It releases automatically on commit/rollback. (ASCII for "llm_conn".)
const ACTIVE_STATE_LOCK: i64 = 0x6c6c6d5f636f6e6e;

fn now() -> Result<String, ApiError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format timestamp"))
}

/// Reject blank label/base_url/model before writing. A connection with an empty
/// base_url or model can never resolve into a usable provider, so it must fail as
/// a 400 rather than being stored as a broken active connection.
fn validate_connection_fields(label: &str, base_url: &str, model: &str) -> Result<(), ApiError> {
    if label.trim().is_empty() {
        return Err(ApiError::bad_request("label is required"));
    }
    if base_url.trim().is_empty() {
        return Err(ApiError::bad_request("base_url is required"));
    }
    if model.trim().is_empty() {
        return Err(ApiError::bad_request("model is required"));
    }
    Ok(())
}

/// Trim surrounding whitespace off the stored identity fields so a padded base_url
/// or model can't silently break request URLs / model routing at call time.
fn normalize_connection_fields(label: &str, base_url: &str, model: &str) -> (String, String, String) {
    (label.trim().to_string(), base_url.trim().to_string(), model.trim().to_string())
}

/// Fields accepted when creating a connection.
#[derive(Debug, Clone)]
pub struct NewConnection {
    pub label: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout_seconds: Option<i64>,
}

/// Fields accepted when updating a connection. `api_key` = None leaves the stored
/// key; `clear_api_key = true` clears it (mirrors clear_provider_api_key).
#[derive(Debug, Clone)]
pub struct UpdateConnection {
    pub label: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub clear_api_key: bool,
    pub model: String,
    pub timeout_seconds: Option<i64>,
}

pub async fn list_connections(pool: &PgPool) -> Result<Vec<ProviderConnection>, ApiError> {
    sqlx::query_as::<_, ProviderConnection>(&format!(
        "SELECT {SELECT_COLUMNS} FROM provider_connections ORDER BY sort_order, created_at"
    ))
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)
}

/// Load the active connection, erroring the same way the old build_provider did
/// when nothing usable is configured.
pub async fn load_active_connection(state: &AppState) -> Result<ActiveConnection, ApiError> {
    let rows = list_connections(&state.pool).await?;
    resolve_active(&rows)
        .map(ActiveConnection::from)
        .ok_or_else(|| ApiError::bad_request("no active provider connection is configured"))
}

pub async fn create_connection(
    pool: &PgPool,
    input: &NewConnection,
) -> Result<ProviderConnection, ApiError> {
    let (label, base_url, model) =
        normalize_connection_fields(&input.label, &input.base_url, &input.model);
    validate_connection_fields(&label, &base_url, &model)?;
    let id = Uuid::new_v4().to_string();
    let ts = now()?;

    // Serialize against activate/delete via the shared active-state lock so two
    // concurrent creates can't both observe an empty table and each insert an
    // is_active=true row (which would break the "exactly one active connection"
    // invariant).
    let mut tx = pool.begin().await.map_err(ApiError::from)?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(ACTIVE_STATE_LOCK)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;

    // First connection becomes active; new ones append after the current max.
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_connections")
        .fetch_one(&mut *tx)
        .await
        .map_err(ApiError::from)?;
    let is_active = existing == 0;
    let next_sort: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM provider_connections",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::from)?;

    sqlx::query(&format!(
        "INSERT INTO provider_connections
           (id, label, base_url, api_key, model, timeout_seconds, is_active, sort_order, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)"
    ))
    .bind(&id)
    .bind(&label)
    .bind(&base_url)
    .bind(input.api_key.as_deref())
    .bind(&model)
    .bind(input.timeout_seconds)
    .bind(is_active)
    .bind(next_sort)
    .bind(&ts)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::from)?;

    tx.commit().await.map_err(ApiError::from)?;

    get_connection(pool, &id)
        .await?
        .ok_or_else(|| ApiError::internal("connection vanished after insert"))
}

async fn get_connection(pool: &PgPool, id: &str) -> Result<Option<ProviderConnection>, ApiError> {
    sqlx::query_as::<_, ProviderConnection>(&format!(
        "SELECT {SELECT_COLUMNS} FROM provider_connections WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)
}

pub async fn update_connection(
    pool: &PgPool,
    id: &str,
    input: &UpdateConnection,
) -> Result<ProviderConnection, ApiError> {
    let (label, base_url, model) =
        normalize_connection_fields(&input.label, &input.base_url, &input.model);
    validate_connection_fields(&label, &base_url, &model)?;
    let ts = now()?;
    let affected = sqlx::query(
        "UPDATE provider_connections
         SET label = $1,
             base_url = $2,
             api_key = COALESCE(NULLIF($3, ''), CASE WHEN $4 THEN NULL ELSE api_key END),
             model = $5,
             timeout_seconds = $6,
             updated_at = $7
         WHERE id = $8",
    )
    .bind(&label)
    .bind(&base_url)
    .bind(input.api_key.as_deref())
    .bind(input.clear_api_key)
    .bind(&model)
    .bind(input.timeout_seconds)
    .bind(&ts)
    .bind(id)
    .execute(pool)
    .await
    .map_err(ApiError::from)?
    .rows_affected();

    if affected == 0 {
        return Err(ApiError::not_found("connection not found"));
    }
    get_connection(pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("connection not found"))
}

/// Activate one connection, clearing is_active on all others in a single UPDATE
/// so exactly one row stays active. The EXISTS guard makes a missing id a no-op
/// (rows_affected == 0 -> not_found) instead of clearing every row's is_active,
/// which would leave the list with zero active connections. Runs under the shared
/// active-state lock so it can't interleave with a concurrent delete/create.
pub async fn activate_connection(pool: &PgPool, id: &str) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(ApiError::from)?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(ACTIVE_STATE_LOCK)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;

    let affected = sqlx::query(
        "UPDATE provider_connections SET is_active = (id = $1)
         WHERE EXISTS (SELECT 1 FROM provider_connections WHERE id = $1)",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::from)?
    .rows_affected();
    if affected == 0 {
        return Err(ApiError::not_found("connection not found"));
    }

    tx.commit().await.map_err(ApiError::from)?;
    Ok(())
}

/// Delete a connection, then unconditionally re-promote the lowest-sort_order
/// survivor whenever no active row remains. Reading "was this active?" before the
/// delete would race a concurrent activate; instead we always heal the invariant
/// after the delete. When the deleted connection was the last one, the table is
/// left empty (an unconfigured state, same as a fresh install) — there is nothing
/// to promote. Runs under the shared active-state lock so it can't interleave with
/// a concurrent activate/create.
pub async fn delete_connection(pool: &PgPool, id: &str) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(ApiError::from)?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(ACTIVE_STATE_LOCK)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;

    let affected = sqlx::query("DELETE FROM provider_connections WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?
        .rows_affected();
    if affected == 0 {
        return Err(ApiError::not_found("connection not found"));
    }

    // Heal the invariant: if nothing is active (either we deleted the active row,
    // or a prior race left the table with zero active), promote the first survivor.
    sqlx::query(
        "UPDATE provider_connections SET is_active = true
         WHERE id = (SELECT id FROM provider_connections ORDER BY sort_order, created_at LIMIT 1)
           AND NOT EXISTS (SELECT 1 FROM provider_connections WHERE is_active)",
    )
    .execute(&mut *tx)
    .await
    .map_err(ApiError::from)?;

    tx.commit().await.map_err(ApiError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(id: &str, active: bool, sort: i32) -> ProviderConnection {
        ProviderConnection {
            id: id.into(),
            label: id.into(),
            base_url: "https://api.example.com".into(),
            api_key: Some("k".into()),
            model: "gpt-4o".into(),
            timeout_seconds: None,
            is_active: active,
            sort_order: sort,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[test]
    fn resolve_active_returns_the_active_row() {
        let rows = vec![conn("a", false, 0), conn("b", true, 1)];
        assert_eq!(resolve_active(&rows).unwrap().id, "b");
    }

    #[test]
    fn resolve_active_none_when_no_active() {
        let rows = vec![conn("a", false, 0)];
        assert!(resolve_active(&rows).is_none());
    }

    #[test]
    fn active_connection_defaults_timeout_and_key() {
        let c = ProviderConnection { api_key: None, timeout_seconds: None, ..conn("a", true, 0) };
        let active = ActiveConnection::from(&c);
        assert_eq!(active.timeout_seconds, 30);
        assert_eq!(active.api_key, "");
    }

    #[test]
    fn validate_connection_fields_rejects_blank_required_fields() {
        assert!(validate_connection_fields("", "https://x", "m").is_err());
        assert!(validate_connection_fields("L", "  ", "m").is_err());
        assert!(validate_connection_fields("L", "https://x", "").is_err());
        assert!(validate_connection_fields("L", "https://x", "m").is_ok());
    }

    #[test]
    fn normalize_connection_fields_trims_surrounding_whitespace() {
        let (label, base_url, model) =
            normalize_connection_fields("  My LLM  ", " https://api.x/v1 ", "  gpt-4o  ");
        assert_eq!(label, "My LLM");
        assert_eq!(base_url, "https://api.x/v1");
        assert_eq!(model, "gpt-4o");
    }
}
