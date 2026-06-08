use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::project::source_identity::{
  source_checkpoint_stem, source_file_name, source_file_stem, source_identity_from_reference,
  source_reference_path, source_summary_path,
};
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
  let source_identity = source_identity_from_reference(file_name);
  let relative = source_reference_path(&source_identity);
  let target = root.safe_join(&relative)?;
  if let Some(parent) = target.parent() {
    fs::create_dir_all(parent)?;
  }
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

  collect_source_entries(root.as_path(), &sources_root, &mut entries)?;

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
  let source_identity = source_identity_from_reference(relative_path);
  let normalized = source_reference_path(&source_identity);
  let target = root.safe_join(&normalized)?;
  if target.exists() {
    fs::remove_file(target)?;
  }
  cleanup_related_wiki_pages(root, &source_identity)?;
  cleanup_source_checkpoints(root, &source_identity)?;
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

fn collect_source_entries(
  project_root: &Path,
  dir: &Path,
  entries: &mut Vec<SourceEntry>,
) -> Result<(), ProjectRootError> {
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      collect_source_entries(project_root, &path, entries)?;
      continue;
    }
    if entry.file_type()?.is_file() {
      entries.push(build_source_entry(project_root, &path)?);
    }
  }

  Ok(())
}

fn cleanup_related_wiki_pages(
  root: &ProjectRoot,
  source_identity: &str,
) -> Result<(), ProjectRootError> {
  let wiki_root = root.safe_join("wiki")?;
  if !wiki_root.exists() {
    return Ok(());
  }

  let related_pages = find_related_wiki_pages(root.as_path(), source_identity)?;
  let mut deleted_refs = Vec::new();

  for page_path in related_pages {
    let content = fs::read_to_string(&page_path)?;
    let sources = parse_sources(&content);
    let decision = decide_page_fate(&sources, source_identity);

    match decision {
      DeleteDecision::Keep { updated_sources } => {
        let updated = write_sources(&content, &updated_sources);
        if updated != content {
          fs::write(&page_path, updated)?;
        }
      }
      DeleteDecision::Delete => {
        deleted_refs.push(page_reference(root.as_path(), &page_path));
        fs::remove_file(&page_path)?;
      }
      DeleteDecision::Skip => {}
    }
  }

  if !deleted_refs.is_empty() {
    cleanup_index_entries(root, &deleted_refs)?;
  }

  Ok(())
}

fn cleanup_source_checkpoints(
  root: &ProjectRoot,
  source_identity: &str,
) -> Result<(), ProjectRootError> {
  let stem = source_checkpoint_stem(source_identity);
  if stem.is_empty() {
    return Ok(());
  }

  let checkpoint_root = root.safe_join(".knowledge/ingest/checkpoints")?;
  for suffix in ["analysis", "generation"] {
    let path = checkpoint_root.join(format!("{stem}.{suffix}.json"));
    if path.exists() {
      fs::remove_file(path)?;
    }
  }

  Ok(())
}

fn cleanup_index_entries(
  root: &ProjectRoot,
  deleted_refs: &[DeletedPageRef],
) -> Result<(), ProjectRootError> {
  let index_path = root.safe_join("wiki/index.md")?;
  if !index_path.exists() {
    return Ok(());
  }

  let existing = fs::read_to_string(&index_path)?;
  let filtered = existing
    .lines()
    .filter(|line| !index_line_matches_deleted_ref(line, deleted_refs))
    .collect::<Vec<_>>()
    .join("\n");

  let normalized = if existing.ends_with('\n') {
    format!("{filtered}\n")
  } else {
    filtered
  };

  if normalized != existing {
    fs::write(index_path, normalized)?;
  }

  Ok(())
}

fn index_line_matches_deleted_ref(line: &str, deleted_refs: &[DeletedPageRef]) -> bool {
  deleted_refs.iter().any(|deleted| {
    line.contains(&format!("[[{}]]", deleted.stem))
      || line.contains(&format!("[[{}]]", deleted.wiki_ref))
  })
}

fn find_related_wiki_pages(
  project_root: &Path,
  source_identity: &str,
) -> Result<Vec<PathBuf>, ProjectRootError> {
  let mut related = Vec::new();
  collect_related_pages(project_root, &project_root.join("wiki"), source_identity, &mut related)?;
  Ok(related)
}

fn collect_related_pages(
  project_root: &Path,
  dir: &Path,
  source_identity: &str,
  results: &mut Vec<PathBuf>,
) -> Result<(), ProjectRootError> {
  let entries = fs::read_dir(dir)?;
  let file_name = source_file_name(source_identity);
  let file_name_lower = file_name.to_lowercase();
  let file_stem = source_file_stem(&file_name);
  let file_stem_lower = if file_stem.is_empty() {
    file_name_lower.clone()
  } else {
    file_stem.to_lowercase()
  };
  let normalized_identity = source_identity.replace('\\', "/");
  let identity_key = normalized_identity.to_lowercase();
  let allow_basename_fallback = !normalized_identity.contains('/');
  let canonical_summary_path = source_summary_path(source_identity);

  for entry in entries {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      collect_related_pages(project_root, &path, source_identity, results)?;
      continue;
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
      continue;
    }

    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
    if matches!(file_name, "index.md" | "log.md" | "overview.md") {
      continue;
    }

    let content = fs::read_to_string(&path)?;
    let sources = parse_sources(&content);
    let sources_match = sources.iter().any(|source| {
      let normalized = source_identity_from_reference(source).to_lowercase();
      normalized == identity_key
        || (allow_basename_fallback
          && !source.contains('/')
          && source_file_name(source).to_lowercase() == file_name_lower)
    });
    let relative_path = path
      .strip_prefix(project_root)
      .unwrap_or(&path)
      .to_string_lossy()
      .replace('\\', "/");
    let is_in_sources_dir = path.components().any(|component| component.as_os_str() == "sources");
    let is_source_summary = relative_path == canonical_summary_path
      || (allow_basename_fallback
        && is_in_sources_dir
        && path
          .file_name()
          .and_then(|name| name.to_str())
          .unwrap_or_default()
          .to_lowercase()
          .starts_with(&file_stem_lower));

    if sources_match || is_source_summary {
      results.push(path);
    }
  }

  Ok(())
}

fn parse_sources(content: &str) -> Vec<String> {
  let Some(frontmatter) = extract_frontmatter(content) else {
    return Vec::new();
  };

  let mut lines = frontmatter.lines().peekable();
  while let Some(line) = lines.next() {
    let trimmed = line.trim();
    if !trimmed.starts_with("sources:") {
      continue;
    }

    let remainder = trimmed.trim_start_matches("sources:").trim();
    if remainder.starts_with('[') && remainder.ends_with(']') {
      return split_inline_array(&remainder[1..remainder.len() - 1]);
    }

    if !remainder.is_empty() {
      return Vec::new();
    }

    let mut values = Vec::new();
    while let Some(next_line) = lines.peek().copied() {
      let next_trimmed = next_line.trim();
      if next_trimmed.is_empty() {
        lines.next();
        continue;
      }
      if !(next_line.starts_with(' ') || next_line.starts_with('\t')) {
        break;
      }
      if let Some(value) = next_trimmed.strip_prefix("- ") {
        values.push(unquote_array_item(value.trim()));
      }
      lines.next();
    }
    return values;
  }

  Vec::new()
}

fn write_sources(content: &str, sources: &[String]) -> String {
  let Some((frontmatter_lines, rest, newline)) = split_frontmatter_lines(content) else {
    return content.to_string();
  };

  let replacement = format!(
    "sources: [{}]",
    sources
      .iter()
      .map(|source| format!("\"{}\"", source.replace('\\', "\\\\").replace('"', "\\\"")))
      .collect::<Vec<_>>()
      .join(", ")
  );

  let mut rewritten = Vec::new();
  let mut replaced = false;
  let mut index = 0usize;

  while index < frontmatter_lines.len() {
    let line = frontmatter_lines[index];
    let trimmed = line.trim();
    if trimmed.starts_with("sources:") {
      rewritten.push(replacement.clone());
      replaced = true;
      index += 1;

      if trimmed.trim_start_matches("sources:").trim().is_empty() {
        while index < frontmatter_lines.len() {
          let next = frontmatter_lines[index];
          let next_trimmed = next.trim();
          if next_trimmed.is_empty() || next.starts_with(' ') || next.starts_with('\t') {
            index += 1;
            continue;
          }
          break;
        }
      }
      continue;
    }

    rewritten.push(line.to_string());
    index += 1;
  }

  if !replaced {
    return content.to_string();
  }

  format!("---{newline}{}{newline}---{newline}{rest}", rewritten.join(newline))
}

fn split_inline_array(body: &str) -> Vec<String> {
  let mut values = Vec::new();
  let mut current = String::new();
  let mut quote = None;
  let mut escaped = false;

  for ch in body.chars() {
    if escaped {
      current.push(ch);
      escaped = false;
      continue;
    }
    if quote == Some('"') && ch == '\\' {
      escaped = true;
      continue;
    }
    if quote.is_none() && matches!(ch, '"' | '\'') {
      quote = Some(ch);
      continue;
    }
    if quote == Some(ch) {
      quote = None;
      continue;
    }
    if ch == ',' && quote.is_none() {
      let value = current.trim();
      if !value.is_empty() {
        values.push(unquote_array_item(value));
      }
      current.clear();
      continue;
    }
    current.push(ch);
  }

  let value = current.trim();
  if !value.is_empty() {
    values.push(unquote_array_item(value));
  }

  values
}

fn unquote_array_item(value: &str) -> String {
  value.trim().trim_matches('"').trim_matches('\'').to_string()
}

fn extract_frontmatter(content: &str) -> Option<&str> {
  split_frontmatter_lines(content).map(|(frontmatter_lines, _, newline)| {
    let joined = frontmatter_lines.join(newline);
    Box::leak(joined.into_boxed_str()) as &str
  })
}

fn split_frontmatter_lines(content: &str) -> Option<(Vec<&str>, &str, &str)> {
  let newline = if content.contains("\r\n") { "\r\n" } else { "\n" };
  let lines = content.lines().collect::<Vec<_>>();
  if lines.first().copied() != Some("---") {
    return None;
  }

  let end_index = lines.iter().enumerate().skip(1).find_map(|(index, line)| {
    if *line == "---" {
      Some(index)
    } else {
      None
    }
  })?;

  let frontmatter_lines = lines[1..end_index].to_vec();
  let rest = lines[end_index + 1..].join(newline);
  Some((frontmatter_lines, Box::leak(rest.into_boxed_str()), newline))
}

fn decide_page_fate(frontmatter_sources: &[String], deleting_source: &str) -> DeleteDecision {
  let deleting_identity = source_identity_from_reference(deleting_source).to_lowercase();
  let deleting_name = source_file_name(deleting_source).to_lowercase();
  let allow_basename_fallback = !deleting_identity.contains('/');
  let in_list = frontmatter_sources.iter().any(|source| {
    let normalized = source_identity_from_reference(source).to_lowercase();
    normalized == deleting_identity
      || (allow_basename_fallback
        && !source.contains('/')
        && source_file_name(source).to_lowercase() == deleting_name)
  });

  if !in_list {
    return DeleteDecision::Skip;
  }

  let survivors = frontmatter_sources
    .iter()
    .filter(|source| {
      let normalized = source_identity_from_reference(source).to_lowercase();
      normalized != deleting_identity
        && !(allow_basename_fallback
          && !source.contains('/')
          && source_file_name(source).to_lowercase() == deleting_name)
    })
    .cloned()
    .collect::<Vec<_>>();

  if survivors.is_empty() {
    DeleteDecision::Delete
  } else {
    DeleteDecision::Keep {
      updated_sources: survivors,
    }
  }
}

fn page_reference(project_root: &Path, path: &Path) -> DeletedPageRef {
  let relative = path
    .strip_prefix(project_root)
    .unwrap_or(path)
    .to_string_lossy()
    .replace('\\', "/");
  let wiki_ref = relative
    .trim_start_matches("wiki/")
    .trim_end_matches(".md")
    .to_string();
  let stem = path
    .file_stem()
    .and_then(|stem| stem.to_str())
    .unwrap_or("")
    .to_string();

  DeletedPageRef { stem, wiki_ref }
}

enum DeleteDecision {
  Keep { updated_sources: Vec<String> },
  Delete,
  Skip,
}

struct DeletedPageRef {
  stem: String,
  wiki_ref: String,
}
