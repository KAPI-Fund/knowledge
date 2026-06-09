use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::service::project_root_for_id;
use crate::projects::tasks::{create_queued_task, CreateTaskRecord};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceWatchSettings {
  pub enabled: bool,
  pub auto_ingest: bool,
  pub path: String,
  pub include_extensions: Vec<String>,
  pub exclude_extensions: Vec<String>,
  pub exclude_dirs: Vec<String>,
  pub exclude_globs: Vec<String>,
  pub max_file_size_mb: i64,
  pub interval_minutes: i64,
  pub last_scan_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceWatchScanResult {
  pub watched_count: usize,
  pub copied_count: usize,
  pub deleted_count: usize,
  pub queued_ingest_count: usize,
  pub queued_delete_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SourceWatchSnapshot {
  files: BTreeMap<String, SourceWatchFileState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SourceWatchFileState {
  hash: String,
  size: u64,
}

impl Default for SourceWatchSettings {
  fn default() -> Self {
    Self {
      enabled: true,
      auto_ingest: true,
      path: String::new(),
      include_extensions: vec![
        "md".to_string(),
        "mdx".to_string(),
        "txt".to_string(),
        "pdf".to_string(),
        "doc".to_string(),
        "docx".to_string(),
        "pptx".to_string(),
        "xls".to_string(),
        "xlsx".to_string(),
        "odt".to_string(),
        "odp".to_string(),
        "ods".to_string(),
        "rtf".to_string(),
        "html".to_string(),
        "htm".to_string(),
        "csv".to_string(),
      ],
      exclude_extensions: vec![
        "tmp".to_string(),
        "temp".to_string(),
        "bak".to_string(),
        "swp".to_string(),
        "part".to_string(),
        "partial".to_string(),
        "crdownload".to_string(),
        "exe".to_string(),
        "dll".to_string(),
        "so".to_string(),
        "dylib".to_string(),
        "bin".to_string(),
        "iso".to_string(),
        "dmg".to_string(),
      ],
      exclude_dirs: vec![
        ".git".to_string(),
        ".svn".to_string(),
        ".hg".to_string(),
        ".obsidian".to_string(),
        ".idea".to_string(),
        ".vscode".to_string(),
        "node_modules".to_string(),
        ".cache".to_string(),
        "__pycache__".to_string(),
      ],
      exclude_globs: vec![
        "~$*".to_string(),
        ".~lock.*#".to_string(),
        "*.draft.*".to_string(),
        "draft-*".to_string(),
        "*.private.*".to_string(),
      ],
      max_file_size_mb: 100,
      interval_minutes: 5,
      last_scan_at: None,
    }
  }
}

pub async fn get_source_watch_settings(
  state: &AppState,
  project_id: &str,
) -> Result<SourceWatchSettings, ApiError> {
  let row = sqlx::query_as::<_, WatchSettingsRow>(
    "SELECT enabled, auto_ingest, path, include_extensions, exclude_extensions, exclude_dirs,
            exclude_globs, max_file_size_mb, interval_minutes, last_scan_at
     FROM project_watch_settings
     WHERE project_id = $1",
  )
  .bind(project_id)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(row.map_or_else(SourceWatchSettings::default, Into::into))
}

pub async fn save_source_watch_settings(
  state: &AppState,
  project_id: &str,
  settings: SourceWatchSettings,
) -> Result<SourceWatchSettings, ApiError> {
  let settings = normalize_source_watch_settings(settings);
  let now = now_rfc3339()?;
  let include_extensions = serde_json::to_string(&settings.include_extensions)
    .map_err(|error| ApiError::internal(error.to_string()))?;
  let exclude_extensions = serde_json::to_string(&settings.exclude_extensions)
    .map_err(|error| ApiError::internal(error.to_string()))?;
  let exclude_dirs =
    serde_json::to_string(&settings.exclude_dirs).map_err(|error| ApiError::internal(error.to_string()))?;
  let exclude_globs =
    serde_json::to_string(&settings.exclude_globs).map_err(|error| ApiError::internal(error.to_string()))?;

  sqlx::query(
    "INSERT INTO project_watch_settings (
       project_id, enabled, auto_ingest, path, include_extensions, exclude_extensions,
       exclude_dirs, exclude_globs, max_file_size_mb, interval_minutes, last_scan_at, created_at,
       updated_at
     ) VALUES (
       $1, $2, $3, $4, $5::jsonb, $6::jsonb, $7::jsonb, $8::jsonb, $9, $10, $11, $12, $13
     )
     ON CONFLICT (project_id) DO UPDATE SET
       enabled = EXCLUDED.enabled,
       auto_ingest = EXCLUDED.auto_ingest,
       path = EXCLUDED.path,
       include_extensions = EXCLUDED.include_extensions,
       exclude_extensions = EXCLUDED.exclude_extensions,
       exclude_dirs = EXCLUDED.exclude_dirs,
       exclude_globs = EXCLUDED.exclude_globs,
       max_file_size_mb = EXCLUDED.max_file_size_mb,
       interval_minutes = EXCLUDED.interval_minutes,
       updated_at = EXCLUDED.updated_at",
  )
  .bind(project_id)
  .bind(settings.enabled)
  .bind(settings.auto_ingest)
  .bind(&settings.path)
  .bind(&include_extensions)
  .bind(&exclude_extensions)
  .bind(&exclude_dirs)
  .bind(&exclude_globs)
  .bind(settings.max_file_size_mb)
  .bind(settings.interval_minutes)
  .bind(settings.last_scan_at.as_deref())
  .bind(&now)
  .bind(&now)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  get_source_watch_settings(state, project_id).await
}

pub async fn scan_project_source_watch(
  state: &AppState,
  project_id: &str,
) -> Result<SourceWatchScanResult, ApiError> {
  let settings = normalize_source_watch_settings(get_source_watch_settings(state, project_id).await?);
  if !settings.enabled || settings.path.is_empty() {
    touch_source_watch_scan_at(state, project_id).await?;
    return Ok(SourceWatchScanResult::default());
  }

  let project_root = project_root_for_id(state, project_id).await?;
  let watch_root = Path::new(&settings.path);
  if !watch_root.is_absolute() {
    return Err(ApiError::bad_request("source watch path must be absolute"));
  }
  if !watch_root.is_dir() {
    return Err(ApiError::bad_request("source watch path must be an existing directory"));
  }

  let snapshot_path = project_root
    .safe_join(".knowledge/source-watch-state.json")
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  let previous_snapshot = load_snapshot(&snapshot_path)?;
  let current_files = collect_watched_files(watch_root, &settings)?;
  let current_paths = current_files.keys().cloned().collect::<BTreeSet<_>>();
  let previous_paths = previous_snapshot.files.keys().cloned().collect::<BTreeSet<_>>();

  let mut result = SourceWatchScanResult {
    watched_count: current_files.len(),
    ..Default::default()
  };

  for relative_path in previous_paths.difference(&current_paths) {
    let raw_relative_path = raw_source_relative_path(relative_path);
    let raw_absolute_path = project_root
      .safe_join(&raw_relative_path)
      .map_err(|error| ApiError::bad_request(error.to_string()))?;
    if raw_absolute_path.exists() {
      fs::remove_file(&raw_absolute_path).map_err(|error| ApiError::internal(error.to_string()))?;
      result.deleted_count += 1;
    }
    enqueue_delete_task(state, project_id, relative_path).await?;
    result.queued_delete_count += 1;
  }

  for (relative_path, current_state) in &current_files {
    let raw_relative_path = raw_source_relative_path(relative_path);
    let raw_absolute_path = project_root
      .safe_join(&raw_relative_path)
      .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let source_absolute_path = watch_root.join(relative_path);
    let previous_state = previous_snapshot.files.get(relative_path);
    let raw_state = read_file_state(&raw_absolute_path)?;
    let should_copy = raw_state.as_ref() != Some(current_state);
    let should_queue = previous_state != Some(current_state) || should_copy || raw_state.is_none();

    if should_copy {
      if let Some(parent) = raw_absolute_path.parent() {
        fs::create_dir_all(parent).map_err(|error| ApiError::internal(error.to_string()))?;
      }
      fs::copy(&source_absolute_path, &raw_absolute_path)
        .map_err(|error| ApiError::internal(error.to_string()))?;
      result.copied_count += 1;
    }

    if should_queue && settings.auto_ingest {
      enqueue_ingest_task(state, project_id, &raw_relative_path).await?;
      result.queued_ingest_count += 1;
    }
  }

  save_snapshot(
    &snapshot_path,
    &SourceWatchSnapshot {
      files: current_files,
    },
  )?;
  touch_source_watch_scan_at(state, project_id).await?;

  Ok(result)
}

pub async fn run_source_watch_tick(state: &AppState) -> Result<usize, ApiError> {
  let rows = sqlx::query_as::<_, (String, i64, Option<String>)>(
    "SELECT project_id, interval_minutes, last_scan_at
     FROM project_watch_settings
     WHERE enabled = TRUE
       AND path <> ''",
  )
  .fetch_all(&state.pool)
  .await
  .map_err(ApiError::from)?;

  let now = OffsetDateTime::now_utc();
  let mut scanned = 0usize;

  for (project_id, interval_minutes, last_scan_at) in rows {
    if !scan_is_due(now, interval_minutes, last_scan_at.as_deref())? {
      continue;
    }
    scan_project_source_watch(state, &project_id).await?;
    scanned += 1;
  }

  Ok(scanned)
}

pub fn spawn_source_watch_scheduler(state: AppState) {
  tokio::spawn(async move {
    loop {
      let _ = run_source_watch_tick(&state).await;
      tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
  });
}

pub async fn touch_source_watch_scan_at(
  state: &AppState,
  project_id: &str,
) -> Result<(), ApiError> {
  let now = now_rfc3339()?;
  sqlx::query(
    "UPDATE project_watch_settings
     SET last_scan_at = $2,
         updated_at = $2
     WHERE project_id = $1",
  )
  .bind(project_id)
  .bind(&now)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;
  Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct WatchSettingsRow {
  enabled: bool,
  auto_ingest: bool,
  path: String,
  include_extensions: Value,
  exclude_extensions: Value,
  exclude_dirs: Value,
  exclude_globs: Value,
  max_file_size_mb: i64,
  interval_minutes: i64,
  last_scan_at: Option<String>,
}

impl From<WatchSettingsRow> for SourceWatchSettings {
  fn from(value: WatchSettingsRow) -> Self {
    Self {
      enabled: value.enabled,
      auto_ingest: value.auto_ingest,
      path: value.path,
      include_extensions: json_array_to_strings(value.include_extensions),
      exclude_extensions: json_array_to_strings(value.exclude_extensions),
      exclude_dirs: json_array_to_strings(value.exclude_dirs),
      exclude_globs: json_array_to_strings(value.exclude_globs),
      max_file_size_mb: value.max_file_size_mb,
      interval_minutes: value.interval_minutes,
      last_scan_at: value.last_scan_at,
    }
  }
}

fn normalize_source_watch_settings(mut settings: SourceWatchSettings) -> SourceWatchSettings {
  settings.path = settings.path.trim().to_string();
  settings.include_extensions = normalize_extensions(settings.include_extensions);
  settings.exclude_extensions = normalize_extensions(settings.exclude_extensions);
  settings.exclude_dirs = normalize_string_list(settings.exclude_dirs);
  settings.exclude_globs = normalize_string_list(settings.exclude_globs);
  settings.max_file_size_mb = settings.max_file_size_mb.clamp(1, 4096);
  settings.interval_minutes = settings.interval_minutes.max(1);
  settings
}

fn normalize_extensions(values: Vec<String>) -> Vec<String> {
  values
    .into_iter()
    .map(|value| value.trim().trim_start_matches('.').to_lowercase())
    .filter(|value| !value.is_empty())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect()
}

fn normalize_string_list(values: Vec<String>) -> Vec<String> {
  values
    .into_iter()
    .map(|value| value.trim().replace('\\', "/"))
    .map(|value| value.trim_matches('/').to_string())
    .filter(|value| !value.is_empty())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect()
}

async fn enqueue_ingest_task(
  state: &AppState,
  project_id: &str,
  raw_relative_path: &str,
) -> Result<(), ApiError> {
  let _ = create_queued_task(
    state,
    CreateTaskRecord {
      project_id: project_id.to_string(),
      task_type: "project.ingest_source".to_string(),
      title: format!("Ingest {raw_relative_path}"),
      relative_path: Some(raw_relative_path.to_string()),
      detail: json!({
        "sourceWatchPath": raw_relative_path
      }),
      created_by: system_actor_id(state).await?,
    },
    json!({
      "relativePath": raw_relative_path
    }),
  )
  .await?;

  Ok(())
}

async fn enqueue_delete_task(
  state: &AppState,
  project_id: &str,
  relative_path: &str,
) -> Result<(), ApiError> {
  let raw_relative_path = raw_source_relative_path(relative_path);
  let _ = create_queued_task(
    state,
    CreateTaskRecord {
      project_id: project_id.to_string(),
      task_type: "project.delete_source".to_string(),
      title: format!("Deleted {relative_path}"),
      relative_path: Some(raw_relative_path),
      detail: json!({
        "sourceWatchPath": relative_path
      }),
      created_by: system_actor_id(state).await?,
    },
    json!({
      "relativePath": relative_path
    }),
  )
  .await?;

  Ok(())
}

async fn system_actor_id(state: &AppState) -> Result<String, ApiError> {
  sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = $1")
    .bind("admin")
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)
}

fn load_snapshot(path: &Path) -> Result<SourceWatchSnapshot, ApiError> {
  let raw = match fs::read_to_string(path) {
    Ok(raw) => raw,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
      return Ok(SourceWatchSnapshot::default());
    }
    Err(error) => return Err(ApiError::internal(error.to_string())),
  };

  serde_json::from_str(&raw).map_err(|error| ApiError::internal(error.to_string()))
}

fn save_snapshot(path: &Path, snapshot: &SourceWatchSnapshot) -> Result<(), ApiError> {
  let serialized =
    serde_json::to_string(snapshot).map_err(|error| ApiError::internal(error.to_string()))?;
  fs::write(path, serialized).map_err(|error| ApiError::internal(error.to_string()))
}

fn collect_watched_files(
  watch_root: &Path,
  settings: &SourceWatchSettings,
) -> Result<BTreeMap<String, SourceWatchFileState>, ApiError> {
  let mut files = BTreeMap::new();
  collect_watched_files_recursive(watch_root, watch_root, settings, &mut files)?;
  Ok(files)
}

fn collect_watched_files_recursive(
  watch_root: &Path,
  current: &Path,
  settings: &SourceWatchSettings,
  files: &mut BTreeMap<String, SourceWatchFileState>,
) -> Result<(), ApiError> {
  for entry in fs::read_dir(current).map_err(|error| ApiError::internal(error.to_string()))? {
    let entry = entry.map_err(|error| ApiError::internal(error.to_string()))?;
    let path = entry.path();
    let file_type = entry
      .file_type()
      .map_err(|error| ApiError::internal(error.to_string()))?;
    let relative = path
      .strip_prefix(watch_root)
      .map_err(|error| ApiError::internal(error.to_string()))?;
    let relative_path = normalize_rel_path(&relative.to_string_lossy());

    if !should_watch_relative_path(&relative_path, file_type.is_dir(), settings) {
      continue;
    }

    if file_type.is_dir() {
      collect_watched_files_recursive(watch_root, &path, settings, files)?;
      continue;
    }

    if file_type.is_file() {
      let metadata = fs::metadata(&path).map_err(|error| ApiError::internal(error.to_string()))?;
      let max_bytes = (settings.max_file_size_mb as u64).saturating_mul(1024 * 1024);
      if metadata.len() > max_bytes {
        continue;
      }
      let state = read_file_state(&path)?
        .ok_or_else(|| ApiError::internal("failed to read watched file state"))?;
      files.insert(relative_path, state);
    }
  }

  Ok(())
}

fn should_watch_relative_path(
  relative_path: &str,
  is_dir: bool,
  settings: &SourceWatchSettings,
) -> bool {
  if relative_path.is_empty() {
    return false;
  }

  let lower = relative_path.to_lowercase();
  if relative_path.split('/').any(|part| part.starts_with('.') || part.is_empty()) {
    return false;
  }

  if settings.exclude_dirs.iter().any(|dir| {
    let dir = dir.to_lowercase();
    relative_path
      .split('/')
      .any(|part| part.eq_ignore_ascii_case(&dir))
  }) {
    return false;
  }

  if settings
    .exclude_globs
    .iter()
    .any(|pattern| wildcard_match(pattern, relative_path) || wildcard_match(pattern, &lower))
  {
    return false;
  }

  if is_dir {
    return true;
  }

  let extension = relative_path
    .rsplit_once('.')
    .map(|(_, ext)| ext.to_lowercase())
    .unwrap_or_default();
  if extension.is_empty() {
    return settings.include_extensions.is_empty();
  }
  if settings.exclude_extensions.iter().any(|item| item == &extension) {
    return false;
  }
  if !settings.include_extensions.is_empty()
    && !settings.include_extensions.iter().any(|item| item == &extension)
  {
    return false;
  }

  true
}

fn read_file_state(path: &Path) -> Result<Option<SourceWatchFileState>, ApiError> {
  if !path.exists() {
    return Ok(None);
  }

  let metadata = fs::metadata(path).map_err(|error| ApiError::internal(error.to_string()))?;
  if !metadata.is_file() {
    return Ok(None);
  }

  let bytes = fs::read(path).map_err(|error| ApiError::internal(error.to_string()))?;
  let mut hasher = Sha256::new();
  hasher.update(&bytes);

  Ok(Some(SourceWatchFileState {
    hash: format!("{:x}", hasher.finalize()),
    size: metadata.len(),
  }))
}

fn raw_source_relative_path(relative_path: &str) -> String {
  format!("raw/sources/{}", normalize_rel_path(relative_path))
}

fn normalize_rel_path(value: &str) -> String {
  value.replace('\\', "/").trim_start_matches('/').to_string()
}

fn json_array_to_strings(value: Value) -> Vec<String> {
  value
    .as_array()
    .map(|items| {
      items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>()
    })
    .unwrap_or_default()
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
  let pattern = pattern.to_lowercase().chars().collect::<Vec<_>>();
  let value = value.to_lowercase().chars().collect::<Vec<_>>();
  wildcard_match_inner(&pattern, &value)
}

fn wildcard_match_inner(pattern: &[char], value: &[char]) -> bool {
  let (mut p, mut v) = (0usize, 0usize);
  let mut star: Option<usize> = None;
  let mut match_after_star = 0usize;

  while v < value.len() {
    if p < pattern.len() && (pattern[p] == '?' || pattern[p] == value[v]) {
      p += 1;
      v += 1;
    } else if p < pattern.len() && pattern[p] == '*' {
      star = Some(p);
      match_after_star = v;
      p += 1;
    } else if let Some(star_pos) = star {
      p = star_pos + 1;
      match_after_star += 1;
      v = match_after_star;
    } else {
      return false;
    }
  }

  while p < pattern.len() && pattern[p] == '*' {
    p += 1;
  }

  p == pattern.len()
}

fn scan_is_due(
  now: OffsetDateTime,
  interval_minutes: i64,
  last_scan_at: Option<&str>,
) -> Result<bool, ApiError> {
  let Some(last_scan_at) = last_scan_at else {
    return Ok(true);
  };

  let last_scan = OffsetDateTime::parse(last_scan_at, &Rfc3339)
    .map_err(|_| ApiError::internal("failed to parse last source watch scan timestamp"))?;
  let elapsed = now - last_scan;
  Ok(elapsed.whole_minutes() >= interval_minutes.max(1))
}

fn now_rfc3339() -> Result<String, ApiError> {
  OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))
}
