use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::project::reviews::maybe_add_review_for_source;
use crate::project::root::ProjectRoot;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestResult {
  pub summary_path: String,
}

#[derive(Debug, Clone)]
#[derive(Serialize)]
pub struct AnalysisResult {
  pub title: String,
  pub summary_markdown: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationCheckpoint {
  summary_path: String,
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
  write_checkpoints(root.as_path(), source_name, analysis, &summary_path)?;
  maybe_add_review_for_source(root, source_name, &analysis.summary_markdown).map_err(io_from_root_error)?;
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

fn write_checkpoints(
  root: &Path,
  source_name: &str,
  analysis: &AnalysisResult,
  summary_path: &str,
) -> Result<(), std::io::Error> {
  let checkpoint_root = root.join(".knowledge/ingest/checkpoints");
  fs::create_dir_all(&checkpoint_root)?;
  let stem = source_name.trim_end_matches(".md");
  let analysis_json = serde_json::to_string(analysis).map_err(std::io::Error::other)?;
  let generation_json = serde_json::to_string(&GenerationCheckpoint {
    summary_path: summary_path.to_string(),
  })
  .map_err(std::io::Error::other)?;
  fs::write(checkpoint_root.join(format!("{stem}.analysis.json")), analysis_json)?;
  fs::write(
    checkpoint_root.join(format!("{stem}.generation.json")),
    generation_json,
  )
}

fn io_from_root_error(error: crate::project::root::ProjectRootError) -> std::io::Error {
  std::io::Error::other(error.to_string())
}
