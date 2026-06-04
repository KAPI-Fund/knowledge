use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::project::root::ProjectRoot;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestResult {
  pub summary_path: String,
}

#[derive(Debug, Clone)]
pub struct AnalysisResult {
  pub title: String,
  pub summary_markdown: String,
}

pub fn analyze_source(source_name: &str, content: &str) -> AnalysisResult {
  let title = content
    .lines()
    .find_map(|line| line.strip_prefix("# ").map(str::trim))
    .filter(|value| !value.is_empty())
    .map(str::to_string)
    .unwrap_or_else(|| source_name.trim_end_matches(".md").to_string());

  AnalysisResult {
    title: title.clone(),
    summary_markdown: content.trim().to_string(),
  }
}

pub fn generate_wiki_from_analysis(
  root: &ProjectRoot,
  source_name: &str,
  analysis: &AnalysisResult,
) -> Result<IngestResult, std::io::Error> {
  let summary_path = format!("wiki/sources/{source_name}");
  let summary_abs = root.safe_join(&summary_path).map_err(io_from_root_error)?;
  let summary = format!(
    "---\ntype: source\ntitle: {}\nsources: [{}]\n---\n\n# {}\n\n{}\n",
    analysis.title, source_name, analysis.title, analysis.summary_markdown
  );
  fs::write(summary_abs, summary)?;
  update_index(root.as_path(), source_name, &analysis.title)?;
  update_log(root.as_path(), &analysis.title)?;
  update_overview(root.as_path(), &analysis.title)?;
  Ok(IngestResult { summary_path })
}

fn update_index(root: &Path, source_name: &str, title: &str) -> Result<(), std::io::Error> {
  let path = root.join("wiki/index.md");
  let existing = fs::read_to_string(&path).unwrap_or_default();
  let slug = source_name.trim_end_matches(".md");
  if existing.contains(&format!("[[{slug}]]")) {
    return Ok(());
  }
  let mut updated = existing;
  updated.push_str(&format!("- [[{slug}]] - {title}\n"));
  fs::write(path, updated)
}

fn update_log(root: &Path, title: &str) -> Result<(), std::io::Error> {
  let path = root.join("wiki/log.md");
  let mut existing = fs::read_to_string(&path).unwrap_or_default();
  existing.push_str(&format!("## 2026-06-04 ingest | {title}\n\n"));
  fs::write(path, existing)
}

fn update_overview(root: &Path, title: &str) -> Result<(), std::io::Error> {
  let path = root.join("wiki/overview.md");
  let content = format!(
    "---\ntype: overview\ntitle: Project Overview\nsources: []\n---\n\n# Overview\n\nThis wiki currently includes {title}.\n"
  );
  fs::write(path, content)
}

fn io_from_root_error(error: crate::project::root::ProjectRootError) -> std::io::Error {
  std::io::Error::other(error.to_string())
}
