// Postgres port of upstream llm_wiki file history
// (upstream_llm_wiki/src-tauri/src/commands/file_history.rs). The upstream
// desktop app stores per-file JSON sidecars; here versions live in the
// file_versions table so multi-tenant pruning and access control apply.

use std::fs;

use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use knowledge_core::project::root::ProjectRoot;

// Upstream file_history.rs L9-10.
pub const MAX_HISTORY_CONTENT_BYTES: u64 = 512 * 1024;
pub const MAX_ENTRIES_PER_FILE: i64 = 30;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileVersionEntry {
    pub id: String,
    pub path: String,
    pub author: String,
    pub tool: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileVersionDetail {
    pub id: String,
    pub path: String,
    pub author: String,
    pub tool: String,
    pub created_at: String,
    pub content: String,
}

#[derive(Debug, thiserror::Error)]
pub enum FileHistoryError {
    #[error("history entry not found")]
    NotFound,
    #[error("history restore is only supported for wiki pages and agent workspace files")]
    UnsupportedPath,
    #[error("failed to restore file: {0}")]
    Restore(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

fn normalize_rel(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches('/').to_string()
}

/// Record the current on-disk state of `rel_path`. Mirrors upstream
/// `record_file_version` (L60-104): skips missing/non-file/oversized targets,
/// dedupes when the latest stored content is identical, prunes to 30 entries.
pub async fn record_disk_version(
    pool: &PgPool,
    project_id: &str,
    root: &ProjectRoot,
    rel_path: &str,
    author: &str,
    tool: &str,
) -> Result<(), sqlx::Error> {
    let rel = normalize_rel(rel_path);
    let Ok(path) = root.safe_join(&rel) else {
        return Ok(());
    };
    let Ok(metadata) = fs::metadata(&path) else {
        return Ok(());
    };
    if !metadata.is_file() || metadata.len() > MAX_HISTORY_CONTENT_BYTES {
        return Ok(());
    }
    let Ok(content) = fs::read_to_string(&path) else {
        return Ok(());
    };
    insert_version(pool, project_id, &rel, author, tool, &content).await
}

/// Record a snapshot from already-captured content (used for pre-write
/// baselines where the old bytes are no longer on disk).
pub async fn record_content_version(
    pool: &PgPool,
    project_id: &str,
    rel_path: &str,
    author: &str,
    tool: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    if content.len() as u64 > MAX_HISTORY_CONTENT_BYTES {
        return Ok(());
    }
    let rel = normalize_rel(rel_path);
    insert_version(pool, project_id, &rel, author, tool, content).await
}

async fn insert_version(
    pool: &PgPool,
    project_id: &str,
    rel_path: &str,
    author: &str,
    tool: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    // Upstream L84: skip when the latest entry already holds this content.
    let latest: Option<(String,)> = sqlx::query_as(
        "SELECT content FROM file_versions
         WHERE project_id = $1 AND path = $2
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
    )
    .bind(project_id)
    .bind(rel_path)
    .fetch_optional(pool)
    .await?;
    if latest.is_some_and(|(existing,)| existing == content) {
        return Ok(());
    }

    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default();
    sqlx::query(
        "INSERT INTO file_versions (id, project_id, path, author, tool, content, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id)
    .bind(rel_path)
    .bind(author)
    .bind(tool)
    .bind(content)
    .bind(&created_at)
    .execute(pool)
    .await?;

    // Upstream L95-97: cap at 30 entries per file.
    sqlx::query(
        "DELETE FROM file_versions
         WHERE project_id = $1 AND path = $2 AND id NOT IN (
           SELECT id FROM file_versions
           WHERE project_id = $1 AND path = $2
           ORDER BY created_at DESC, id DESC
           LIMIT $3
         )",
    )
    .bind(project_id)
    .bind(rel_path)
    .bind(MAX_ENTRIES_PER_FILE)
    .execute(pool)
    .await?;
    Ok(())
}

/// Newest first, metadata only (upstream `list_file_history` L120-134 returns
/// full content; the server keeps payloads small and serves content by id).
pub async fn list_file_history(
    pool: &PgPool,
    project_id: &str,
    rel_path: &str,
) -> Result<Vec<FileVersionEntry>, sqlx::Error> {
    let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, path, author, tool, created_at FROM file_versions
         WHERE project_id = $1 AND path = $2
         ORDER BY created_at DESC, id DESC",
    )
    .bind(project_id)
    .bind(normalize_rel(rel_path))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, path, author, tool, created_at)| FileVersionEntry {
            id,
            path,
            author,
            tool,
            created_at,
        })
        .collect())
}

pub async fn get_file_history_entry(
    pool: &PgPool,
    project_id: &str,
    entry_id: &str,
) -> Result<Option<FileVersionDetail>, sqlx::Error> {
    let row: Option<(String, String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, path, author, tool, created_at, content FROM file_versions
         WHERE project_id = $1 AND id = $2",
    )
    .bind(project_id)
    .bind(entry_id)
    .fetch_optional(pool)
    .await?;
    Ok(
        row.map(|(id, path, author, tool, created_at, content)| FileVersionDetail {
            id,
            path,
            author,
            tool,
            created_at,
            content,
        }),
    )
}

pub fn is_restorable_history_path(rel_path: &str) -> bool {
    let rel = normalize_rel(rel_path);
    if !knowledge_core::project::files::is_public_project_rel(&rel) {
        return false;
    }
    let lower = rel.to_lowercase();
    (lower.starts_with("wiki/") && lower.ends_with(".md")) || lower.starts_with("agent-workspace/")
}

/// Upstream `restore_file_history` (L137-157): write the entry content back,
/// then record the restore itself so the timeline shows it.
pub async fn restore_file_version(
    pool: &PgPool,
    project_id: &str,
    root: &ProjectRoot,
    entry_id: &str,
    author: &str,
) -> Result<FileVersionDetail, FileHistoryError> {
    let entry = get_file_history_entry(pool, project_id, entry_id)
        .await?
        .ok_or(FileHistoryError::NotFound)?;
    if !is_restorable_history_path(&entry.path) {
        return Err(FileHistoryError::UnsupportedPath);
    }
    let target = root
        .safe_join(&entry.path)
        .map_err(|error| FileHistoryError::Restore(error.to_string()))?;
    if fs::symlink_metadata(&target).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(FileHistoryError::Restore(
            "refusing to overwrite a symlink".to_string(),
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| FileHistoryError::Restore(error.to_string()))?;
    }
    fs::write(&target, &entry.content)
        .map_err(|error| FileHistoryError::Restore(error.to_string()))?;
    record_disk_version(pool, project_id, root, &entry.path, author, "history.restore").await?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restorable_paths_are_wiki_markdown_or_workspace() {
        assert!(is_restorable_history_path("wiki/index.md"));
        assert!(is_restorable_history_path("agent-workspace/report/out.svg"));
        assert!(!is_restorable_history_path("wiki/media/image.png"));
        assert!(!is_restorable_history_path("raw/sources/book.md"));
        assert!(!is_restorable_history_path("purpose.md"));
        assert!(!is_restorable_history_path("wiki/../purpose.md"));
        assert!(!is_restorable_history_path("agent-workspace/.hidden"));
    }
}
