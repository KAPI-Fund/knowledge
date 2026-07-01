use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::http::error::ApiError;

fn now_rfc3339() -> Result<String, ApiError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format timestamp"))
}

pub fn asset_url(id: &str) -> String {
    format!("/api/assets/{id}")
}

pub struct NewAsset {
    pub id: String,
    pub owner_id: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

impl NewAsset {
    pub fn new(owner_id: &str, mime: &str, bytes: Vec<u8>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            owner_id: owner_id.to_string(),
            mime: mime.to_string(),
            bytes,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AssetRecord {
    pub id: String,
    pub owner_id: String,
    pub mime: String,
    pub bytes: Vec<u8>,
    pub created_at: String,
}

pub async fn insert_asset(pool: &PgPool, asset: &NewAsset) -> Result<String, ApiError> {
    let now = now_rfc3339()?;
    sqlx::query(
        "INSERT INTO assets (id, owner_id, mime, bytes, created_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&asset.id)
    .bind(&asset.owner_id)
    .bind(&asset.mime)
    .bind(&asset.bytes)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(ApiError::from)?;
    Ok(asset.id.clone())
}

pub async fn get_asset(
    pool: &PgPool,
    id: &str,
    owner_id: &str,
) -> Result<Option<AssetRecord>, ApiError> {
    sqlx::query_as::<_, AssetRecord>(
        "SELECT id, owner_id, mime, bytes, created_at
         FROM assets
         WHERE id = $1 AND owner_id = $2",
    )
    .bind(id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_url_uses_api_path() {
        assert_eq!(asset_url("abc123"), "/api/assets/abc123");
    }

    #[test]
    fn new_asset_generates_id_and_keeps_mime() {
        let asset = NewAsset::new("user1", "image/png", vec![1, 2, 3]);
        assert_eq!(asset.owner_id, "user1");
        assert_eq!(asset.mime, "image/png");
        assert_eq!(asset.bytes, vec![1, 2, 3]);
        assert!(!asset.id.is_empty());
    }
}
