use std::fs;

use serde::Serialize;
use time::OffsetDateTime;

use crate::project::root::{ProjectRoot, ProjectRootError};

#[derive(Debug, Clone)]
pub struct SaveQueryPageInput {
  pub title: String,
  pub slug: String,
  pub answer: String,
  pub citations: Vec<SavedQueryCitation>,
  pub context_summary: String,
}

#[derive(Debug, Clone)]
pub struct SavedQueryCitation {
  pub path: String,
  pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveQueryPageResult {
  pub relative_path: String,
}

pub fn save_query_page(
  root: &ProjectRoot,
  input: SaveQueryPageInput,
) -> Result<SaveQueryPageResult, ProjectRootError> {
  let relative_path = format!("wiki/queries/{}.md", input.slug);
  let absolute_path = root.safe_join(&relative_path)?;
  fs::write(&absolute_path, render_query_page(&input))?;
  update_index(root.as_path(), &input.slug, &input.title)?;
  update_log(root.as_path(), &input.title)?;

  Ok(SaveQueryPageResult { relative_path })
}

fn render_query_page(input: &SaveQueryPageInput) -> String {
  let sources = render_sources_frontmatter(&input.citations);
  let citations = render_citations(&input.citations);
  let context = if input.context_summary.trim().is_empty() {
    "No context summary recorded.".to_string()
  } else {
    input.context_summary.clone()
  };

  format!(
    "---\ntype: query\ntitle: {}\n{}\n---\n\n# {}\n\n{}\n\n## Sources\n\n{}\n\n## Context\n\n{}\n",
    input.title,
    sources,
    input.title,
    input.answer.trim(),
    citations,
    context
  )
}

fn render_sources_frontmatter(citations: &[SavedQueryCitation]) -> String {
  if citations.is_empty() {
    return "sources: []".to_string();
  }

  let mut lines = vec!["sources:".to_string()];
  for citation in citations {
    lines.push(format!("  - {}", citation.path));
  }
  lines.join("\n")
}

fn render_citations(citations: &[SavedQueryCitation]) -> String {
  if citations.is_empty() {
    return "- None".to_string();
  }

  citations
    .iter()
    .map(|citation| format!("- {} - {}", wiki_link(&citation.path), citation.title))
    .collect::<Vec<_>>()
    .join("\n")
}

fn wiki_link(path: &str) -> String {
  let trimmed = path.trim_start_matches("wiki/").trim_end_matches(".md");
  format!("[[{trimmed}]]")
}

fn update_index(root: &std::path::Path, slug: &str, title: &str) -> Result<(), std::io::Error> {
  let path = root.join("wiki/index.md");
  let existing = fs::read_to_string(&path).unwrap_or_default();
  let entry = format!("- [[queries/{slug}]] - {title}\n");

  if existing.contains(&entry.trim_end().to_string()) {
    return Ok(());
  }

  let marker = "## Queries\n";
  let updated = if let Some(index) = existing.find(marker) {
    let insert_at = index + marker.len();
    let mut updated = existing.clone();
    updated.insert_str(insert_at, &entry);
    updated
  } else {
    format!("{existing}\n## Queries\n{entry}")
  };

  fs::write(path, updated)
}

fn update_log(root: &std::path::Path, title: &str) -> Result<(), std::io::Error> {
  let path = root.join("wiki/log.md");
  let mut existing = fs::read_to_string(&path).unwrap_or_default();
  let date = OffsetDateTime::now_utc().date().to_string();
  existing.push_str(&format!("## {date} query | {title}\n\n"));
  fs::write(path, existing)
}
