use std::fs;
use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
  pub path: String,
  pub title: String,
  pub snippet: String,
  pub score: usize,
}

pub fn search_project(project_root: &Path, query: &str) -> Result<Vec<SearchResult>, std::io::Error> {
  let wiki_root = project_root.join("wiki");
  let mut results = Vec::new();
  let query_tokens = tokenize_query(query);

  if !wiki_root.exists() {
    return Ok(results);
  }

  walk_markdown(&wiki_root, &mut |path| {
    let content = fs::read_to_string(path)?;
    let score = query_tokens
      .iter()
      .filter(|token| content.to_lowercase().contains(token.as_str()))
      .count();

    if score == 0 {
      return Ok(());
    }

    let snippet = build_snippet(&content, query);
    let relative = path
      .strip_prefix(project_root)
      .unwrap_or(path)
      .to_string_lossy()
      .replace('\\', "/");

    results.push(SearchResult {
      path: relative,
      title: extract_title(&content, path.file_name().and_then(|name| name.to_str()).unwrap_or("")),
      snippet,
      score,
    });

    Ok(())
  })?;

  results.sort_by(|left, right| right.score.cmp(&left.score).then(left.path.cmp(&right.path)));
  Ok(results)
}

pub fn tokenize_query(query: &str) -> Vec<String> {
  query
    .to_lowercase()
    .split(|ch: char| !ch.is_alphanumeric())
    .filter(|token| token.len() > 1)
    .map(str::to_string)
    .collect()
}

pub fn extract_title(content: &str, file_name: &str) -> String {
  content
    .lines()
    .find_map(|line| line.strip_prefix("# ").map(str::trim))
    .filter(|title| !title.is_empty())
    .map(str::to_string)
    .unwrap_or_else(|| file_name.trim_end_matches(".md").to_string())
}

pub fn build_snippet(content: &str, query: &str) -> String {
  let compact = content.replace('\n', " ");
  let lower = compact.to_lowercase();
  let query_lower = query.to_lowercase();
  if let Some(index) = lower.find(&query_lower) {
    let start = index.saturating_sub(30);
    let end = (index + query_lower.len() + 30).min(compact.len());
    compact[start..end].trim().to_string()
  } else {
    compact.chars().take(80).collect()
  }
}

fn walk_markdown(
  root: &Path,
  visit: &mut impl FnMut(&Path) -> Result<(), std::io::Error>,
) -> Result<(), std::io::Error> {
  for entry in fs::read_dir(root)? {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      walk_markdown(&path, visit)?;
    } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
      visit(&path)?;
    }
  }

  Ok(())
}
