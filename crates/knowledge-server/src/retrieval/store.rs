use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::http::error::ApiError;

#[derive(Debug, Clone)]
pub struct StoredChunkInput {
  pub page_id: String,
  pub relative_path: String,
  pub title: String,
  pub embedding_model: String,
  pub chunk_index: u32,
  pub heading_path: String,
  pub chunk_text: String,
  pub content_hash: String,
  pub embedding: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct StoredChunk {
  pub page_id: String,
  pub relative_path: String,
  pub title: String,
  pub embedding_model: String,
  pub chunk_index: u32,
  pub heading_path: String,
  pub chunk_text: String,
  pub content_hash: String,
  pub embedding: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct PageIndexState {
  pub content_hash: String,
  pub embedding_model: String,
}

pub async fn load_page_hashes(
  pool: &sqlx::PgPool,
  project_id: &str,
) -> Result<BTreeMap<String, PageIndexState>, ApiError> {
  let rows = sqlx::query_as::<_, (String, String, String)>(
    "SELECT DISTINCT page_id, content_hash, embedding_model
     FROM project_embedding_chunks
     WHERE project_id = $1",
  )
  .bind(project_id)
  .fetch_all(pool)
  .await
  .map_err(ApiError::from)?;

  Ok(
    rows
      .into_iter()
      .map(|(page_id, content_hash, embedding_model)| {
        (
          page_id,
          PageIndexState {
            content_hash,
            embedding_model,
          },
        )
      })
      .collect(),
  )
}

pub async fn replace_page_chunks(
  pool: &sqlx::PgPool,
  project_id: &str,
  page_id: &str,
  chunks: &[StoredChunkInput],
) -> Result<(), ApiError> {
  let mut tx = pool.begin().await.map_err(ApiError::from)?;

  sqlx::query(
    "DELETE FROM project_embedding_chunks
     WHERE project_id = $1 AND page_id = $2",
  )
  .bind(project_id)
  .bind(page_id)
  .execute(&mut *tx)
  .await
  .map_err(ApiError::from)?;

  let timestamp = now_rfc3339()?;
  for chunk in chunks {
    sqlx::query(
      "INSERT INTO project_embedding_chunks (
         id,
         project_id,
         page_id,
         relative_path,
         title,
         embedding_model,
         chunk_index,
         heading_path,
         chunk_text,
         content_hash,
       embedding,
        created_at,
        updated_at
       ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id)
    .bind(&chunk.page_id)
    .bind(&chunk.relative_path)
    .bind(&chunk.title)
    .bind(&chunk.embedding_model)
    .bind(i64::from(chunk.chunk_index))
    .bind(&chunk.heading_path)
    .bind(&chunk.chunk_text)
    .bind(&chunk.content_hash)
    .bind(json!(chunk.embedding))
    .bind(&timestamp)
    .bind(&timestamp)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::from)?;
  }

  tx.commit().await.map_err(ApiError::from)?;
  Ok(())
}

pub async fn delete_missing_pages(
  pool: &sqlx::PgPool,
  project_id: &str,
  keep_page_ids: &BTreeSet<String>,
) -> Result<(), ApiError> {
  let rows = sqlx::query_scalar::<_, String>(
    "SELECT DISTINCT page_id
     FROM project_embedding_chunks
     WHERE project_id = $1",
  )
  .bind(project_id)
  .fetch_all(pool)
  .await
  .map_err(ApiError::from)?;

  for page_id in rows {
    if keep_page_ids.contains(&page_id) {
      continue;
    }

    sqlx::query(
      "DELETE FROM project_embedding_chunks
       WHERE project_id = $1 AND page_id = $2",
    )
    .bind(project_id)
    .bind(page_id)
    .execute(pool)
    .await
    .map_err(ApiError::from)?;
  }

  Ok(())
}

pub async fn delete_pages(
  pool: &sqlx::PgPool,
  project_id: &str,
  page_ids: &[String],
) -> Result<(), ApiError> {
  for page_id in page_ids {
    sqlx::query(
      "DELETE FROM project_embedding_chunks
       WHERE project_id = $1 AND page_id = $2",
    )
    .bind(project_id)
    .bind(page_id)
    .execute(pool)
    .await
    .map_err(ApiError::from)?;
  }

  Ok(())
}

pub async fn load_project_chunks(
  pool: &sqlx::PgPool,
  project_id: &str,
) -> Result<Vec<StoredChunk>, ApiError> {
  let rows = sqlx::query_as::<_, (String, String, String, String, i32, String, String, String, Value)>(
    "SELECT
       page_id,
       relative_path,
       title,
       embedding_model,
       chunk_index,
       heading_path,
       chunk_text,
       content_hash,
       embedding
     FROM project_embedding_chunks
     WHERE project_id = $1",
  )
  .bind(project_id)
  .fetch_all(pool)
  .await
  .map_err(ApiError::from)?;

  rows
    .into_iter()
    .map(
      |(page_id, relative_path, title, embedding_model, chunk_index, heading_path, chunk_text, content_hash, embedding)| {
        Ok(StoredChunk {
          page_id,
          relative_path,
          title,
          embedding_model,
          chunk_index: u32::try_from(chunk_index)
            .map_err(|_| ApiError::internal("invalid stored chunk index"))?,
          heading_path,
          chunk_text,
          content_hash,
          embedding: parse_embedding(&embedding)?,
        })
      },
    )
    .collect()
}

fn parse_embedding(value: &Value) -> Result<Vec<f32>, ApiError> {
  let Some(items) = value.as_array() else {
    return Err(ApiError::internal("stored embedding is not an array"));
  };

  items
    .iter()
    .map(|item| {
      let number = item
        .as_f64()
        .ok_or_else(|| ApiError::internal("stored embedding contains a non-number"))?;
      if !number.is_finite() {
        return Err(ApiError::internal("stored embedding contains a non-finite value"));
      }
      Ok(number as f32)
    })
    .collect()
}

fn now_rfc3339() -> Result<String, ApiError> {
  OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))
}
