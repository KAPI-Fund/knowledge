use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::project::root::{ProjectRoot, ProjectRootError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceEntry {
  pub relative_path: String,
  pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTaskRecord {
  pub task_type: String,
  pub relative_path: String,
  pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourceTaskQueue {
  pub tasks: Vec<SourceTaskRecord>,
}

pub fn import_source(
  root: &ProjectRoot,
  file_name: &str,
  content_base64: &str,
) -> Result<SourceEntry, ProjectRootError> {
  let relative = format!("raw/sources/{file_name}");
  let target = root.safe_join(&relative)?;
  let bytes = STANDARD
    .decode(content_base64)
    .map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))?;
  fs::write(&target, bytes)?;
  append_task(root, "source_import", &relative)?;
  build_source_entry(root.as_path(), &target)
}

pub fn list_sources(root: &ProjectRoot) -> Result<Vec<SourceEntry>, ProjectRootError> {
  let sources_root = root.safe_join("raw/sources")?;
  let mut entries = Vec::new();

  if !sources_root.exists() {
    return Ok(entries);
  }

  for entry in fs::read_dir(sources_root)? {
    let entry = entry?;
    if entry.file_type()?.is_file() {
      entries.push(build_source_entry(root.as_path(), &entry.path())?);
    }
  }

  entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
  Ok(entries)
}

pub fn rescan_sources(root: &ProjectRoot) -> Result<usize, ProjectRootError> {
  let sources = list_sources(root)?;
  for entry in &sources {
    append_task(root, "source_rescan", &entry.relative_path)?;
  }
  Ok(sources.len())
}

pub fn delete_source(root: &ProjectRoot, relative_path: &str) -> Result<(), ProjectRootError> {
  let normalized = format!("raw/sources/{relative_path}");
  let target = root.safe_join(&normalized)?;
  if target.exists() {
    fs::remove_file(target)?;
  }
  append_task(root, "source_delete", &normalized)?;
  Ok(())
}

pub fn load_queue(root: &ProjectRoot) -> Result<SourceTaskQueue, ProjectRootError> {
  let queue_path = root.safe_join(".knowledge/ingest/queue.json")?;
  let raw = fs::read_to_string(queue_path)?;
  serde_json::from_str(&raw).map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))
}

fn append_task(
  root: &ProjectRoot,
  task_type: &str,
  relative_path: &str,
) -> Result<(), ProjectRootError> {
  let queue_path = root.safe_join(".knowledge/ingest/queue.json")?;
  let mut queue = if queue_path.exists() {
    load_queue(root)?
  } else {
    SourceTaskQueue::default()
  };

  queue.tasks.push(SourceTaskRecord {
    task_type: task_type.to_string(),
    relative_path: relative_path.to_string(),
    created_at: OffsetDateTime::now_utc()
      .format(&Rfc3339)
      .map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))?,
  });

  let json = serde_json::to_string(&queue)
    .map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))?;
  fs::write(queue_path, json)?;
  Ok(())
}

fn build_source_entry(root: &Path, path: &PathBuf) -> Result<SourceEntry, ProjectRootError> {
  let metadata = fs::metadata(path)?;
  let relative_path = path
    .strip_prefix(root)
    .map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))?
    .to_string_lossy()
    .replace('\\', "/");

  Ok(SourceEntry {
    relative_path,
    size: metadata.len(),
  })
}
