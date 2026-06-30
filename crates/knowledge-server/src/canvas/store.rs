use serde::Serialize;
use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::http::error::ApiError;

use super::document::{default_canvas_document, CanvasDocument};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CanvasRecord {
    pub id: String,
    pub owner_id: String,
    pub title: String,
    pub document: String,
    pub created_at: String,
    pub updated_at: String,
}

impl CanvasRecord {
    pub fn parse_document(&self) -> Result<CanvasDocument, serde_json::Error> {
        serde_json::from_str(&self.document)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CanvasSummary {
    pub id: String,
    pub title: String,
    pub updated_at: String,
}

impl From<&CanvasRecord> for CanvasSummary {
    fn from(rec: &CanvasRecord) -> Self {
        Self {
            id: rec.id.clone(),
            title: rec.title.clone(),
            updated_at: rec.updated_at.clone(),
        }
    }
}

fn now_rfc3339() -> Result<String, ApiError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format timestamp"))
}

pub async fn create_canvas(
    pool: &PgPool,
    owner_id: &str,
    title: &str,
) -> Result<CanvasRecord, ApiError> {
    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339()?;
    let document =
        serde_json::to_string(&default_canvas_document()).unwrap_or_else(|_| "{}".into());
    sqlx::query(
        "INSERT INTO canvases (id, owner_id, title, document, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&id)
    .bind(owner_id)
    .bind(title)
    .bind(&document)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(ApiError::from)?;

    Ok(CanvasRecord {
        id,
        owner_id: owner_id.to_string(),
        title: title.to_string(),
        document,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn list_canvases(
    pool: &PgPool,
    owner_id: &str,
) -> Result<Vec<CanvasRecord>, ApiError> {
    sqlx::query_as::<_, CanvasRecord>(
        "SELECT id, owner_id, title, document, created_at, updated_at
         FROM canvases
         WHERE owner_id = $1
         ORDER BY updated_at DESC",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)
}

pub async fn get_canvas(
    pool: &PgPool,
    id: &str,
    owner_id: &str,
) -> Result<Option<CanvasRecord>, ApiError> {
    sqlx::query_as::<_, CanvasRecord>(
        "SELECT id, owner_id, title, document, created_at, updated_at
         FROM canvases
         WHERE id = $1 AND owner_id = $2",
    )
    .bind(id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)
}

pub async fn update_canvas(
    pool: &PgPool,
    id: &str,
    owner_id: &str,
    title: &str,
    document: &str,
) -> Result<Option<CanvasRecord>, ApiError> {
    let now = now_rfc3339()?;
    let affected = sqlx::query(
        "UPDATE canvases
         SET title = $1, document = $2, updated_at = $3
         WHERE id = $4 AND owner_id = $5",
    )
    .bind(title)
    .bind(document)
    .bind(&now)
    .bind(id)
    .bind(owner_id)
    .execute(pool)
    .await
    .map_err(ApiError::from)?
    .rows_affected();

    if affected == 0 {
        return Ok(None);
    }
    get_canvas(pool, id, owner_id).await
}

pub async fn delete_canvas(
    pool: &PgPool,
    id: &str,
    owner_id: &str,
) -> Result<bool, ApiError> {
    let affected =
        sqlx::query("DELETE FROM canvases WHERE id = $1 AND owner_id = $2")
            .bind(id)
            .bind(owner_id)
            .execute(pool)
            .await
            .map_err(ApiError::from)?
            .rows_affected();
    Ok(affected > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_document_reads_text_json() {
        let rec = CanvasRecord {
            id: "c1".into(),
            owner_id: "u1".into(),
            title: "Board".into(),
            document: r#"{"nodes":[],"edges":[],"viewport":{"x":0,"y":0,"zoom":1}}"#.into(),
            created_at: "t1".into(),
            updated_at: "t2".into(),
        };
        let doc = rec.parse_document().expect("valid json");
        assert!(doc.nodes.is_empty());
        assert_eq!(doc.viewport.zoom, 1.0);
    }

    #[test]
    fn parse_document_errs_on_bad_json() {
        let rec = CanvasRecord {
            id: "c1".into(),
            owner_id: "u1".into(),
            title: "Board".into(),
            document: "not json".into(),
            created_at: "t1".into(),
            updated_at: "t2".into(),
        };
        assert!(rec.parse_document().is_err());
    }

    #[test]
    fn summary_from_record_drops_document() {
        let rec = CanvasRecord {
            id: "c1".into(),
            owner_id: "u1".into(),
            title: "Board".into(),
            document: "{}".into(),
            created_at: "t1".into(),
            updated_at: "t2".into(),
        };
        let summary = CanvasSummary::from(&rec);
        assert_eq!(summary.id, "c1");
        assert_eq!(summary.title, "Board");
        assert_eq!(summary.updated_at, "t2");
    }
}
