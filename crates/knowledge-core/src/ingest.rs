use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use unicode_normalization::UnicodeNormalization;

use crate::project::reviews::{maybe_add_review_for_source, save_generated_reviews};
use crate::project::root::ProjectRoot;

const FILE_OPENER_PREFIX: &str = "---FILE:";
const FILE_CLOSER_CANONICAL: &str = "---END FILE---";
const MAX_SOURCE_SUMMARY_SLUG_LENGTH: usize = 120;
const FALLBACK_SOURCE_PART: &str = "source";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestResult {
  pub summary_path: String,
  pub cache_hit: bool,
  pub written_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
  pub title: String,
  pub summary_markdown: String,
  pub analysis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerationCheckpoint {
  summary_path: String,
  written_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IngestCacheEntry {
  hash: String,
  summary_path: String,
  written_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct IngestCacheStore {
  entries: BTreeMap<String, IngestCacheEntry>,
}

#[derive(Debug, Clone)]
struct ParsedFileBlock {
  path: String,
  content: String,
}

pub fn analyze_source(source_name: &str, content: &str) -> AnalysisResult {
  AnalysisResult {
    title: source_title(source_name, content),
    summary_markdown: content.trim().to_string(),
    analysis: String::new(),
  }
}

pub fn source_summary_path(source_identity: &str) -> String {
  format!(
    "wiki/sources/{}.md",
    source_summary_slug_from_identity(source_identity)
  )
}

pub fn build_analysis_prompt(purpose: &str, index: &str, source_content: &str) -> String {
  [
    "You are an expert research analyst. Read the source document and produce a structured analysis.",
    "Do not output chain-of-thought, hidden reasoning, or a thinking transcript. Reason internally and write only the concise final analysis.",
    "",
    "Your analysis should cover:",
    "",
    "## Key Entities",
    "- Name and type",
    "- Role in the source",
    "- Whether it likely already exists in the wiki",
    "",
    "## Key Concepts",
    "- Name and brief definition",
    "- Why it matters in this source",
    "- Whether it likely already exists in the wiki",
    "",
    "## Main Arguments & Findings",
    "- Core claims or results",
    "- Supporting evidence",
    "- Evidence strength",
    "",
    "## Connections to Existing Wiki",
    "- Related existing pages",
    "- Whether the source strengthens, challenges, or extends existing knowledge",
    "",
    "## Contradictions & Tensions",
    "- Conflicts with existing wiki content",
    "- Internal tensions or caveats",
    "",
    "## Recommendations",
    "- Pages that should be created or updated",
    "- What to emphasize or de-emphasize",
    "- Open questions worth flagging",
    "",
    "Be thorough but concise. Focus on what is genuinely important.",
    "",
    &optional_section("## Wiki Purpose (for context)", purpose),
    &optional_section("## Current Wiki Index (for checking existing content)", index),
    &optional_section("## Source Language Hint", detect_language_hint(source_content)),
  ]
  .into_iter()
  .filter(|section| !section.is_empty())
  .collect::<Vec<_>>()
  .join("\n")
}

pub fn build_analysis_user_prompt(source_name: &str, source_content: &str) -> String {
  format!(
    "Analyze this source document:\n\n**File:** {source_name}\n\n---\n\n{source_content}"
  )
}

pub fn build_generation_prompt(
  schema: &str,
  purpose: &str,
  index: &str,
  source_file_name: &str,
  overview: &str,
  source_content: &str,
  summary_path: &str,
) -> String {
  let today = current_date_string();
  [
    "You are a wiki maintainer. Based on the analysis provided, generate wiki files.",
    "Do not output chain-of-thought, hidden reasoning, or explanatory preamble. Reason internally and output only the requested FILE or REVIEW blocks.",
    "",
    "## IMPORTANT: Source File",
    &format!("The original source file is: **{source_file_name}**"),
    "All wiki pages generated from this source MUST include this filename in their frontmatter `sources` field.",
    &format!(
      "Today's date is **{today}**. Use this exact date for all new `created`, `updated`, and wiki/log.md ingest dates."
    ),
    "",
    &optional_section("## Project Schema and Routing (AUTHORITATIVE)", schema),
    "",
    "## What to generate",
    "",
    &format!("1. A source summary page at **{summary_path}** (MUST use this exact path)"),
    "2. Entity or concept pages for key named things and ideas identified in the analysis",
    "3. An updated wiki/index.md",
    "4. A log entry inside wiki/log.md",
    "5. An updated wiki/overview.md",
    "",
    "## Frontmatter Rules",
    "Every page begins with a YAML frontmatter block.",
    "Arrays use the standard YAML inline form.",
    &format!("Every generated page must include `sources: [\"{source_file_name}\"]`."),
    "",
    "## Output Format",
    "Your entire response consists of FILE blocks followed by optional REVIEW blocks. Nothing else.",
    "FILE block template:",
    "```",
    "---FILE: wiki/path/to/page.md---",
    "(complete file content with YAML frontmatter)",
    "---END FILE---",
    "```",
    "",
    "Output Requirements:",
    "1. The first character of your response must be `-`.",
    "2. Do not output preamble or commentary.",
    "3. Do not restate the analysis outside FILE blocks.",
    "4. Every FILE block path must live under wiki/.",
    "",
    &optional_section("## Wiki Purpose", purpose),
    &optional_section("## Current Wiki Index", index),
    &optional_section("## Current Overview", overview),
    &optional_section("## Source Language Hint", detect_language_hint(source_content)),
  ]
  .into_iter()
  .filter(|section| !section.is_empty())
  .collect::<Vec<_>>()
  .join("\n")
}

pub fn build_generation_user_prompt(
  source_name: &str,
  analysis: &str,
  source_content: &str,
) -> String {
  [
    &format!("Source document to process: **{source_name}**"),
    "",
    "The Stage 1 analysis below is context to inform your output. Do not echo it.",
    "",
    "## Stage 1 Analysis",
    analysis,
    "",
    "## Source Context",
    source_content,
    "",
    "---",
    "",
    &format!("Now emit the FILE blocks for the wiki files derived from **{source_name}**."),
    "Your response must begin with `---FILE:` as the very first characters.",
  ]
  .join("\n")
}

pub fn check_ingest_cache(
  root: &ProjectRoot,
  source_identity: &str,
  source_content: &str,
) -> Result<Option<IngestResult>, std::io::Error> {
  let path = cache_path(root);
  let store = read_cache_store(&path)?;
  let Some(entry) = store.entries.get(source_identity) else {
    return Ok(None);
  };

  if entry.hash != sha256_hex(source_content) {
    return Ok(None);
  }

  for relative in &entry.written_paths {
    let absolute = root.safe_join(relative).map_err(io_from_root_error)?;
    if !absolute.exists() {
      return Ok(None);
    }
  }

  Ok(Some(IngestResult {
    summary_path: entry.summary_path.clone(),
    cache_hit: true,
    written_paths: entry.written_paths.clone(),
  }))
}

pub fn generate_wiki_from_analysis(
  root: &ProjectRoot,
  source_identity: &str,
  source_name: &str,
  source_content: &str,
  analysis: &AnalysisResult,
  generation_text: &str,
) -> Result<IngestResult, std::io::Error> {
  let default_summary_path = source_summary_path(source_identity);
  let parsed_blocks = parse_file_blocks(generation_text);
  let mut written_paths = Vec::new();
  let mut wrote_summary = false;
  let mut wrote_index = false;
  let mut wrote_log = false;
  let mut wrote_overview = false;

  for block in parsed_blocks {
    let absolute = root.safe_join(&block.path).map_err(io_from_root_error)?;
    let existing_content = if absolute.exists() {
      fs::read_to_string(&absolute).ok()
    } else {
      None
    };
    if let Some(parent) = absolute.parent() {
      fs::create_dir_all(parent)?;
    }
    let merged_content = merge_array_fields_into_content(
      &block.content,
      existing_content.as_deref(),
      &["sources", "tags", "related"],
    );
    fs::write(&absolute, merged_content)?;
    if !written_paths.contains(&block.path) {
      written_paths.push(block.path.clone());
    }

    wrote_summary |= block.path == default_summary_path;
    wrote_index |= block.path == "wiki/index.md";
    wrote_log |= block.path == "wiki/log.md";
    wrote_overview |= block.path == "wiki/overview.md";
  }

  if !wrote_summary {
    let summary_content = fallback_summary_content(source_name, analysis);
    let summary_abs = root.safe_join(&default_summary_path).map_err(io_from_root_error)?;
    if let Some(parent) = summary_abs.parent() {
      fs::create_dir_all(parent)?;
    }
    fs::write(summary_abs, summary_content)?;
    if !written_paths.contains(&default_summary_path) {
      written_paths.push(default_summary_path.clone());
    }
  }

  if !wrote_index {
    update_index(root.as_path(), source_identity, &analysis.title)?;
    if !written_paths.contains(&"wiki/index.md".to_string()) {
      written_paths.push("wiki/index.md".to_string());
    }
  }

  if !wrote_log {
    update_log(root.as_path(), &analysis.title)?;
    if !written_paths.contains(&"wiki/log.md".to_string()) {
      written_paths.push("wiki/log.md".to_string());
    }
  }

  if !wrote_overview {
    update_overview(root.as_path(), &analysis.title)?;
    if !written_paths.contains(&"wiki/overview.md".to_string()) {
      written_paths.push("wiki/overview.md".to_string());
    }
  }

  let generated_review_count =
    save_generated_reviews(root, source_name, generation_text).map_err(io_from_root_error)?;
  if generated_review_count == 0 {
    maybe_add_review_for_source(root, source_name, source_content).map_err(io_from_root_error)?;
  }

  let result = IngestResult {
    summary_path: default_summary_path.clone(),
    cache_hit: false,
    written_paths,
  };

  write_checkpoints(root.as_path(), source_name, analysis, &result)?;
  save_ingest_cache(root, source_identity, source_content, &result)?;

  Ok(result)
}

fn fallback_summary_content(source_name: &str, analysis: &AnalysisResult) -> String {
  format!(
    "---\ntype: source\ntitle: {}\nsources: [\"{}\"]\n---\n\n# {}\n\n{}\n",
    analysis.title,
    source_name,
    analysis.title,
    if analysis.analysis.trim().is_empty() {
      analysis.summary_markdown.trim()
    } else {
      analysis.analysis.trim()
    }
  )
}

fn update_index(root: &Path, source_name: &str, title: &str) -> Result<(), std::io::Error> {
  let path = root.join("wiki/index.md");
  let existing = fs::read_to_string(&path).unwrap_or_default();
  let slug = source_summary_slug_from_identity(source_name);
  let summary_entry = format!("- [[{slug}]] - {title}");
  if existing.contains(&summary_entry) {
    return Ok(());
  }

  let marker = "## Sources\n";
  let updated = if let Some(index) = existing.find(marker) {
    let insert_at = index + marker.len();
    let mut value = existing;
    value.insert_str(insert_at, &format!("{summary_entry}\n"));
    value
  } else {
    format!("{existing}\n## Sources\n{summary_entry}\n")
  };
  fs::write(path, updated)
}

fn update_log(root: &Path, title: &str) -> Result<(), std::io::Error> {
  let path = root.join("wiki/log.md");
  let mut existing = fs::read_to_string(&path).unwrap_or_default();
  if !existing.ends_with('\n') && !existing.is_empty() {
    existing.push('\n');
  }
  existing.push_str(&format!(
    "## {} ingest | {}\n\n",
    current_date_string(),
    title
  ));
  fs::write(path, existing)
}

fn update_overview(root: &Path, title: &str) -> Result<(), std::io::Error> {
  let path = root.join("wiki/overview.md");
  let content = format!(
    "---\ntype: overview\ntitle: Project Overview\ntags: []\nrelated: []\nsources: []\n---\n\n# Overview\n\nThis wiki covers {}.\n",
    title
  );
  fs::write(path, content)
}

fn write_checkpoints(
  root: &Path,
  source_name: &str,
  analysis: &AnalysisResult,
  result: &IngestResult,
) -> Result<(), std::io::Error> {
  let checkpoint_root = root.join(".knowledge/ingest/checkpoints");
  fs::create_dir_all(&checkpoint_root)?;
  let stem = source_name_stem(source_name);
  let analysis_json = serde_json::to_string(analysis).map_err(std::io::Error::other)?;
  let generation_json = serde_json::to_string(&GenerationCheckpoint {
    summary_path: result.summary_path.clone(),
    written_paths: result.written_paths.clone(),
  })
  .map_err(std::io::Error::other)?;
  fs::write(checkpoint_root.join(format!("{stem}.analysis.json")), analysis_json)?;
  fs::write(
    checkpoint_root.join(format!("{stem}.generation.json")),
    generation_json,
  )
}

fn parse_file_blocks(text: &str) -> Vec<ParsedFileBlock> {
  let normalized = text.replace("\r\n", "\n");
  let lines = normalized.lines().collect::<Vec<_>>();
  let mut blocks = Vec::new();
  let mut index = 0usize;
  let mut fence_marker = None;
  let mut fence_len = 0usize;

  while index < lines.len() {
    let Some(path) = parse_file_opener(lines[index]) else {
      index += 1;
      continue;
    };
    index += 1;

    let mut content_lines = Vec::new();
    while index < lines.len() {
      let line = lines[index];
      if let Some((marker, len)) = parse_fence_line(line) {
        match fence_marker {
          None => {
            fence_marker = Some(marker);
            fence_len = len;
          }
          Some(existing) if existing == marker && len >= fence_len => {
            fence_marker = None;
            fence_len = 0;
          }
          _ => {}
        }
        content_lines.push(line);
        index += 1;
        continue;
      }

      if fence_marker.is_none() && is_file_closer(line) {
        index += 1;
        break;
      }

      content_lines.push(line);
      index += 1;
    }

    if !is_safe_ingest_path(&path) {
      continue;
    }

    blocks.push(ParsedFileBlock {
      path,
      content: content_lines.join("\n"),
    });
  }

  blocks
}

fn parse_file_opener(line: &str) -> Option<String> {
  let trimmed = line.trim();
  if !trimmed.to_ascii_uppercase().starts_with(FILE_OPENER_PREFIX) || !trimmed.ends_with("---") {
    return None;
  }

  let path = trimmed
    .strip_prefix(FILE_OPENER_PREFIX)?
    .strip_suffix("---")?
    .trim();
  if path.is_empty() {
    return None;
  }
  Some(path.to_string())
}

fn is_file_closer(line: &str) -> bool {
  line
    .trim()
    .chars()
    .filter(|ch| !ch.is_whitespace())
    .collect::<String>()
    .eq_ignore_ascii_case(&FILE_CLOSER_CANONICAL.replace(' ', ""))
}

fn parse_fence_line(line: &str) -> Option<(char, usize)> {
  let trimmed = line.trim_start();
  let first = trimmed.chars().next()?;
  if first != '`' && first != '~' {
    return None;
  }

  let run_length = trimmed.chars().take_while(|ch| *ch == first).count();
  if run_length < 3 {
    return None;
  }

  Some((first, run_length))
}

fn is_safe_ingest_path(path: &str) -> bool {
  if path.trim().is_empty() || path.chars().any(|ch| ch.is_control()) {
    return false;
  }
  if path.starts_with('/') || path.starts_with('\\') || path.starts_with("//") {
    return false;
  }
  if path.len() > 1 && path.as_bytes()[1] == b':' {
    return false;
  }

  let normalized = path.replace('\\', "/");
  if !normalized.starts_with("wiki/") {
    return false;
  }

  for segment in normalized.split('/') {
    if segment.is_empty()
      || segment == ".."
      || segment.ends_with(' ')
      || segment.ends_with('.')
      || segment.chars().any(|ch| matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
      return false;
    }
  }

  true
}

fn merge_array_fields_into_content(
  new_content: &str,
  existing_content: Option<&str>,
  fields: &[&str],
) -> String {
  let Some(existing_content) = existing_content else {
    return new_content.to_string();
  };
  if !existing_content.starts_with("---") || !new_content.starts_with("---") {
    return new_content.to_string();
  }

  let mut result = new_content.to_string();
  let mut changed = false;

  for field in fields {
    let old_values = parse_frontmatter_array(existing_content, field);
    if old_values.is_empty() {
      continue;
    }

    let new_values = parse_frontmatter_array(&result, field);
    let merged = merge_frontmatter_lists(&old_values, &new_values);
    if merged == new_values {
      continue;
    }

    result = write_frontmatter_array(&result, field, &merged);
    changed = true;
  }

  if changed { result } else { new_content.to_string() }
}

fn parse_frontmatter_array(content: &str, field_name: &str) -> Vec<String> {
  let Some((frontmatter_lines, _, _)) = split_frontmatter_lines_owned(content) else {
    return Vec::new();
  };

  let field_prefix = format!("{field_name}:");
  let mut index = 0usize;
  while index < frontmatter_lines.len() {
    let line = &frontmatter_lines[index];
    let trimmed = line.trim();
    if !trimmed.starts_with(&field_prefix) {
      index += 1;
      continue;
    }

    let remainder = trimmed[field_prefix.len()..].trim();
    if remainder.starts_with('[') && remainder.ends_with(']') {
      return split_inline_array(&remainder[1..remainder.len() - 1]);
    }

    if !remainder.is_empty() {
      return Vec::new();
    }

    let mut values = Vec::new();
    index += 1;
    while index < frontmatter_lines.len() {
      let next_line = &frontmatter_lines[index];
      let next_trimmed = next_line.trim();
      if next_trimmed.is_empty() {
        index += 1;
        continue;
      }
      if !(next_line.starts_with(' ') || next_line.starts_with('\t')) {
        break;
      }
      if let Some(value) = next_trimmed.strip_prefix("- ") {
        values.push(unquote_inline_array_item(value.trim()));
      }
      index += 1;
    }
    return values;
  }

  Vec::new()
}

fn write_frontmatter_array(content: &str, field_name: &str, values: &[String]) -> String {
  let Some((frontmatter_lines, rest, newline)) = split_frontmatter_lines_owned(content) else {
    return content.to_string();
  };

  let replacement = format!(
    "{field_name}: [{}]",
    values
      .iter()
      .map(|value| format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")))
      .collect::<Vec<_>>()
      .join(", ")
  );
  let field_prefix = format!("{field_name}:");
  let mut rewritten = Vec::new();
  let mut replaced = false;
  let mut index = 0usize;

  while index < frontmatter_lines.len() {
    let line = &frontmatter_lines[index];
    let trimmed = line.trim();
    if trimmed.starts_with(&field_prefix) {
      rewritten.push(replacement.clone());
      replaced = true;
      index += 1;

      if trimmed[field_prefix.len()..].trim().is_empty() {
        while index < frontmatter_lines.len() {
          let next_line = &frontmatter_lines[index];
          let next_trimmed = next_line.trim();
          if next_trimmed.is_empty() || next_line.starts_with(' ') || next_line.starts_with('\t') {
            index += 1;
            continue;
          }
          break;
        }
      }
      continue;
    }

    rewritten.push(line.clone());
    index += 1;
  }

  if !replaced {
    rewritten.push(replacement);
  }

  if rest.is_empty() {
    format!("---{newline}{}{newline}---", rewritten.join(newline))
  } else {
    format!("---{newline}{}{newline}---{newline}{rest}", rewritten.join(newline))
  }
}

fn merge_frontmatter_lists(existing: &[String], incoming: &[String]) -> Vec<String> {
  let mut merged = Vec::new();
  let mut seen = std::collections::BTreeSet::new();

  for value in existing.iter().chain(incoming.iter()) {
    let key = value.to_lowercase();
    if seen.insert(key) {
      merged.push(value.clone());
    }
  }

  merged
}

fn split_frontmatter_lines_owned(content: &str) -> Option<(Vec<String>, String, &'static str)> {
  let newline = if content.contains("\r\n") { "\r\n" } else { "\n" };
  let lines = content.split(newline).collect::<Vec<_>>();
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

  let frontmatter_lines = lines[1..end_index]
    .iter()
    .map(|line| (*line).to_string())
    .collect::<Vec<_>>();
  let rest = lines[end_index + 1..].join(newline);
  Some((frontmatter_lines, rest, newline))
}

fn unquote_inline_array_item(value: &str) -> String {
  value.trim().trim_matches('"').trim_matches('\'').to_string()
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
        values.push(unquote_inline_array_item(value));
      }
      current.clear();
      continue;
    }
    current.push(ch);
  }

  let value = current.trim();
  if !value.is_empty() {
    values.push(unquote_inline_array_item(value));
  }

  values
}

fn cache_path(root: &ProjectRoot) -> std::path::PathBuf {
  root
    .safe_join(".knowledge/ingest/cache.json")
    .unwrap_or_else(|_| root.as_path().join(".knowledge/ingest/cache.json"))
}

fn read_cache_store(path: &Path) -> Result<IngestCacheStore, std::io::Error> {
  if !path.exists() {
    return Ok(IngestCacheStore::default());
  }

  let raw = fs::read_to_string(path)?;
  serde_json::from_str(&raw).map_err(std::io::Error::other)
}

fn save_ingest_cache(
  root: &ProjectRoot,
  source_identity: &str,
  source_content: &str,
  result: &IngestResult,
) -> Result<(), std::io::Error> {
  let path = cache_path(root);
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  let mut store = read_cache_store(&path)?;
  store.entries.insert(
    source_identity.to_string(),
    IngestCacheEntry {
      hash: sha256_hex(source_content),
      summary_path: result.summary_path.clone(),
      written_paths: result.written_paths.clone(),
    },
  );
  let raw = serde_json::to_string(&store).map_err(std::io::Error::other)?;
  fs::write(path, raw)
}

fn sha256_hex(content: &str) -> String {
  let mut hasher = Sha256::new();
  hasher.update(content.as_bytes());
  format!("{:x}", hasher.finalize())
}

fn source_title(source_name: &str, content: &str) -> String {
  content
    .lines()
    .find_map(|line| line.strip_prefix("# ").map(str::trim))
    .filter(|value| !value.is_empty())
    .map(str::to_string)
    .unwrap_or_else(|| source_name_stem(source_name).replace('-', " "))
}

fn source_name_stem(source_name: &str) -> String {
  Path::new(source_name)
    .file_stem()
    .and_then(|value| value.to_str())
    .unwrap_or(source_name)
    .to_string()
}

fn source_summary_slug_from_identity(source_identity: &str) -> String {
  let normalized = source_identity.replace('\\', "/");
  let without_extension = if let Some((parent, file_name)) = normalized.rsplit_once('/') {
    format!("{parent}/{}", strip_source_extension(file_name))
  } else {
    strip_source_extension(&normalized).to_string()
  };
  let parts = without_extension
    .split('/')
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>();

  if parts.len() <= 1 {
    return parts.first().copied().unwrap_or(FALLBACK_SOURCE_PART).to_string();
  }

  let hash = stable_slug_hash(source_identity);
  let prefix = parts
    .iter()
    .map(|part| {
      let (readable, structural_length) = readable_slug_part(part);
      format!("{structural_length}-{readable}")
    })
    .collect::<Vec<_>>()
    .join("--");

  let full_slug = format!("{prefix}--{hash}");
  if full_slug.chars().count() <= MAX_SOURCE_SUMMARY_SLUG_LENGTH {
    return full_slug;
  }

  let readable_limit = MAX_SOURCE_SUMMARY_SLUG_LENGTH.saturating_sub(hash.chars().count() + 2);
  let readable_prefix = prefix
    .chars()
    .take(readable_limit)
    .collect::<String>()
    .trim_end_matches('-')
    .to_string();
  format!(
    "{}--{hash}",
    if readable_prefix.is_empty() {
      FALLBACK_SOURCE_PART
    } else {
      readable_prefix.as_str()
    }
  )
}

fn strip_source_extension(file_name: &str) -> &str {
  match file_name.rsplit_once('.') {
    Some((prefix, _)) if !prefix.is_empty() => prefix,
    _ => file_name,
  }
}

fn readable_slug_part(part: &str) -> (String, usize) {
  let normalized = part.nfkc().collect::<String>();
  let trimmed = normalized.trim();
  let mut structural = String::new();
  let mut last_was_dash = false;

  for ch in trimmed.chars() {
    if ch.is_whitespace() {
      if !structural.is_empty() && !last_was_dash {
        structural.push('-');
        last_was_dash = true;
      }
      continue;
    }

    let mut wrote = false;
    for lower in ch.to_lowercase() {
      if lower.is_alphanumeric() || lower == '-' {
        structural.push(lower);
        last_was_dash = lower == '-';
        wrote = true;
      }
    }
    if !wrote && !structural.is_empty() && !last_was_dash {
      last_was_dash = false;
    }
  }

  let structural = structural.trim_matches('-').to_string();
  let readable = structural.replace("--", "-");
  let readable = collapse_repeated_hyphens(&readable);
  let readable = if readable.is_empty() {
    FALLBACK_SOURCE_PART.to_string()
  } else {
    readable
  };
  let structural_length = if structural.is_empty() {
    FALLBACK_SOURCE_PART.chars().count()
  } else {
    structural.chars().count()
  };
  (readable, structural_length.max(1))
}

fn collapse_repeated_hyphens(value: &str) -> String {
  let mut output = String::new();
  let mut last_was_dash = false;
  for ch in value.chars() {
    if ch == '-' {
      if !last_was_dash {
        output.push(ch);
      }
      last_was_dash = true;
    } else {
      output.push(ch);
      last_was_dash = false;
    }
  }
  output
}

fn stable_slug_hash(value: &str) -> String {
  let mut hash: u32 = 0x811c9dc5;
  for byte in value.as_bytes() {
    hash ^= u32::from(*byte);
    hash = hash.wrapping_mul(0x01000193);
  }
  radix36(hash)
}

fn radix36(mut value: u32) -> String {
  if value == 0 {
    return "0".to_string();
  }

  let mut output = Vec::new();
  while value > 0 {
    let digit = (value % 36) as u8;
    output.push(match digit {
      0..=9 => (b'0' + digit) as char,
      _ => (b'a' + (digit - 10)) as char,
    });
    value /= 36;
  }
  output.iter().rev().collect()
}

fn optional_section(title: &str, content: impl AsRef<str>) -> String {
  let content = content.as_ref().trim();
  if content.is_empty() {
    String::new()
  } else {
    format!("{title}\n{content}")
  }
}

fn detect_language_hint(source_content: &str) -> &'static str {
  if source_content
    .chars()
    .any(|ch| matches!(ch, '\u{4e00}'..='\u{9fff}'))
  {
    "Prefer Chinese output when appropriate."
  } else {
    "Prefer English output."
  }
}

fn current_date_string() -> String {
  let now = OffsetDateTime::now_utc().date();
  format!(
    "{:04}-{:02}-{:02}",
    now.year(),
    u8::from(now.month()),
    now.day()
  )
}

fn io_from_root_error(error: crate::project::root::ProjectRootError) -> std::io::Error {
  std::io::Error::other(error.to_string())
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use crate::project::scaffold::initialize_project;

  use super::{source_summary_path, AnalysisResult, generate_wiki_from_analysis};

  #[test]
  fn repeated_ingest_merges_sources_tags_and_related_arrays() {
    let temp = tempdir().unwrap();
    let root = initialize_project(temp.path()).unwrap();
    let concept_path = temp.path().join("wiki/concepts/attention-mechanism.md");

    std::fs::write(
      &concept_path,
      [
        "---",
        "type: concept",
        "title: Attention Mechanism",
        "tags: [alpha]",
        "related: [origin-page]",
        "sources: [\"first.md\"]",
        "---",
        "",
        "# Attention Mechanism",
        "",
        "Existing content.",
      ]
      .join("\n"),
    )
    .unwrap();

    let generation = [
      "---FILE: wiki/concepts/attention-mechanism.md---",
      "---",
      "type: concept",
      "title: Attention Mechanism",
      "tags: [beta]",
      "related: [follow-up-page]",
      "sources: [\"second.md\"]",
      "---",
      "",
      "# Attention Mechanism",
      "",
      "Fresh content.",
      "---END FILE---",
    ]
    .join("\n");

    let result = generate_wiki_from_analysis(
      &root,
      "second.md",
      "second.md",
      "# Second\n\nFresh content.",
      &AnalysisResult {
        title: "Second".to_string(),
        summary_markdown: "Fresh content.".to_string(),
        analysis: "Fresh content.".to_string(),
      },
      &generation,
    )
    .unwrap();

    assert!(
      result
        .written_paths
        .iter()
        .any(|path| path == "wiki/concepts/attention-mechanism.md")
    );

    let merged = std::fs::read_to_string(concept_path).unwrap();
    assert!(merged.contains("sources: [\"first.md\", \"second.md\"]"));
    assert!(merged.contains("tags: [\"alpha\", \"beta\"]"));
    assert!(merged.contains("related: [\"origin-page\", \"follow-up-page\"]"));
    assert!(merged.contains("Fresh content."));
  }

  #[test]
  fn source_summary_path_keeps_root_level_basename_for_non_markdown_sources() {
    assert_eq!(source_summary_path("config.yaml"), "wiki/sources/config.md");
    assert_eq!(source_summary_path("attention.docx"), "wiki/sources/attention.md");
  }

  #[test]
  fn source_summary_path_uses_structural_nested_slug_with_hash() {
    let first = source_summary_path("a--b/config.yaml");
    let second = source_summary_path("a/b/config.yaml");
    assert!(first.starts_with("wiki/sources/4-a-b--6-config--"));
    assert!(second.starts_with("wiki/sources/1-a--1-b--6-config--"));
    assert_ne!(first, second);
    assert!(first.len() <= 136);
  }
}
