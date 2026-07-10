use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::project::root::{ProjectRoot, ProjectRootError};

// upstream_llm_wiki/src/lib/lint.ts L20-26
const BROKEN_LINK_SUGGESTION_MIN_SCORE: f64 = 0.74;
const RELATED_PAGE_SUGGESTION_MIN_SCORE: f64 = 0.08;
const SAME_FOLDER_SCORE_BONUS: f64 = 0.08;
const SINGLE_CJK_TOKEN_WEIGHT: f64 = 0.35;
const SUGGESTION_TOKEN_WINDOW: usize = 4000;
const SAME_BASENAME_SCORE: f64 = 0.96;
const CONTAINS_TARGET_SCORE: f64 = 0.82;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StructuralLintIssue {
  pub issue_type: String,
  pub severity: String,
  pub page: String,
  pub detail: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub broken_target: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub suggested_target: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub suggested_source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuralLintResult {
  pub mode: String,
  pub issues: Vec<StructuralLintIssue>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticLintIssue {
  pub issue_type: String,
  pub severity: String,
  pub page: String,
  pub detail: String,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub affected_pages: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticLintResult {
  pub mode: String,
  pub issues: Vec<SemanticLintIssue>,
}

#[derive(Debug, Clone)]
struct PageData {
  page: String,
  slug: String,
  title: String,
  outlinks: Vec<String>,
  tokens: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuggestDirection {
  Source,
  Target,
}

pub fn run_structural_lint(root: &ProjectRoot) -> Result<StructuralLintResult, ProjectRootError> {
  let wiki_root = root.safe_join("wiki")?;
  if !wiki_root.exists() {
    return Ok(StructuralLintResult {
      mode: "structural".to_string(),
      issues: Vec::new(),
    });
  }

  let markdown_files = collect_markdown_files(&wiki_root)?;
  let content_files = markdown_files
    .into_iter()
    .filter(|path| !matches!(path.file_name().and_then(|name| name.to_str()), Some("index.md" | "log.md")))
    .collect::<Vec<_>>();
  let slug_map = build_slug_map(&wiki_root, &content_files);

  let pages = content_files
    .iter()
    .map(|path| load_page_data(&wiki_root, path))
    .collect::<Result<Vec<_>, _>>()?;

  let inbound_counts = build_inbound_counts(&slug_map, &pages);
  let mut issues = Vec::new();

  for page in &pages {
    let inbound = inbound_counts.get(&page.slug.to_lowercase()).copied().unwrap_or(0);
    if inbound == 0 {
      // upstream_llm_wiki/src/lib/lint.ts L253-264
      let suggested_source = suggest_related_page(&pages, page, SuggestDirection::Source);
      issues.push(StructuralLintIssue {
        issue_type: "orphan".to_string(),
        severity: "info".to_string(),
        page: page.page.clone(),
        detail: "No other pages link to this page.".to_string(),
        broken_target: None,
        suggested_target: None,
        suggested_source: suggested_source.map(|candidate| candidate.page.clone()),
      });
    }

    if page.outlinks.is_empty() {
      // upstream_llm_wiki/src/lib/lint.ts L266-276
      let suggested_target = suggest_related_page(&pages, page, SuggestDirection::Target);
      issues.push(StructuralLintIssue {
        issue_type: "no-outlinks".to_string(),
        severity: "info".to_string(),
        page: page.page.clone(),
        detail: "This page has no [[wikilink]] references to other pages.".to_string(),
        broken_target: None,
        suggested_target: suggested_target.map(|candidate| candidate.page.clone()),
        suggested_source: None,
      });
    }

    for link in &page.outlinks {
      let lookup = link.to_lowercase();
      let basename = file_name_without_md(link).to_lowercase();
      let normalized = normalize_slug_key(link);
      if slug_map.contains_key(&lookup)
        || slug_map.contains_key(&basename)
        || slug_map.contains_key(&normalized)
      {
        continue;
      }

      // upstream_llm_wiki/src/lib/lint.ts L278-294
      let suggested_target = suggest_broken_target(&pages, link);
      issues.push(StructuralLintIssue {
        issue_type: "broken-link".to_string(),
        severity: "warning".to_string(),
        page: page.page.clone(),
        detail: format!("Broken link: [[{link}]] - target page not found."),
        broken_target: Some(link.clone()),
        suggested_target: suggested_target.map(|candidate| candidate.page.clone()),
        suggested_source: None,
      });
    }
  }

  Ok(StructuralLintResult {
    mode: "structural".to_string(),
    issues,
  })
}

pub fn build_semantic_lint_prompt(root: &ProjectRoot) -> Result<Option<String>, ProjectRootError> {
  let wiki_root = root.safe_join("wiki")?;
  if !wiki_root.exists() {
    return Ok(None);
  }

  let markdown_files = collect_markdown_files(&wiki_root)?;
  let wiki_files = markdown_files
    .into_iter()
    .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some("log.md"))
    .collect::<Vec<_>>();

  let mut summaries = Vec::new();
  for path in wiki_files {
    let content = fs::read_to_string(&path)?;
    let preview = if content.chars().count() > 500 {
      let truncated = content.chars().take(500).collect::<String>();
      format!("{truncated}...")
    } else {
      content
    };
    let short_path = path
      .strip_prefix(&wiki_root)
      .unwrap_or(&path)
      .to_string_lossy()
      .replace('\\', "/");
    summaries.push(format!("### {short_path}\n{preview}"));
  }

  if summaries.is_empty() {
    return Ok(None);
  }

  Ok(Some(
    [
      "You are a wiki quality analyst. Review the following wiki page summaries and identify issues.",
      "",
      "For each issue, output exactly this format:",
      "",
      "---LINT: type | severity | Short title---",
      "Description of the issue.",
      "PAGES: page1.md, page2.md",
      "---END LINT---",
      "",
      "Types:",
      "- contradiction: two or more pages make conflicting claims",
      "- stale: information that appears outdated or superseded",
      "- missing-page: an important concept is heavily referenced but has no dedicated page",
      "- suggestion: a question or source worth adding to the wiki",
      "",
      "Severities:",
      "- warning: should be addressed",
      "- info: nice to have",
      "",
      "Only report genuine issues. Do not invent problems. Output ONLY the ---LINT--- blocks, no other text.",
      "",
      "## Wiki Pages",
      "",
      &summaries.join("\n\n"),
    ]
    .join("\n"),
  ))
}

pub fn parse_semantic_lint_response(raw: &str) -> SemanticLintResult {
  let mut issues = Vec::new();
  let normalized = raw.replace("\r\n", "\n");
  let mut cursor = normalized.as_str();

  while let Some(start) = cursor.find("---LINT:") {
    let block = &cursor[start + "---LINT:".len()..];
    let Some(end) = block.find("---END LINT---") else {
      break;
    };
    let inner = block[..end].trim();
    let Some((header, body)) = inner.split_once('\n') else {
      cursor = &block[end + "---END LINT---".len()..];
      continue;
    };

    let header_parts = header
      .split('|')
      .map(|value| value.trim().trim_end_matches('-').trim())
      .filter(|value| !value.is_empty())
      .collect::<Vec<_>>();
    if header_parts.len() != 3 {
      cursor = &block[end + "---END LINT---".len()..];
      continue;
    }

    let raw_type = header_parts[0];
    let severity = if header_parts[1].eq_ignore_ascii_case("warning") {
      "warning"
    } else {
      "info"
    };
    let title = header_parts[2];
    let affected_pages = body
      .lines()
      .find_map(|line| line.trim().strip_prefix("PAGES:"))
      .map(|value| {
        value
          .split(',')
          .map(str::trim)
          .filter(|item| !item.is_empty())
          .map(str::to_string)
          .collect::<Vec<_>>()
      })
      .unwrap_or_default();
    let detail = body
      .lines()
      .filter(|line| !line.trim_start().starts_with("PAGES:"))
      .collect::<Vec<_>>()
      .join("\n")
      .trim()
      .to_string();

    issues.push(SemanticLintIssue {
      issue_type: "semantic".to_string(),
      severity: severity.to_string(),
      page: title.to_string(),
      detail: format!("[{raw_type}] {detail}"),
      affected_pages,
    });
    cursor = &block[end + "---END LINT---".len()..];
  }

  SemanticLintResult {
    mode: "semantic".to_string(),
    issues,
  }
}

fn collect_markdown_files(root: &Path) -> Result<Vec<PathBuf>, ProjectRootError> {
  let mut files = Vec::new();
  walk_markdown(root, &mut files)?;
  Ok(files)
}

fn walk_markdown(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), ProjectRootError> {
  for entry in fs::read_dir(root)? {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      walk_markdown(&path, files)?;
    } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
      files.push(path);
    }
  }

  Ok(())
}

fn build_slug_map(wiki_root: &Path, files: &[PathBuf]) -> BTreeMap<String, String> {
  let mut slug_map = BTreeMap::new();
  for path in files {
    let relative = path
      .strip_prefix(wiki_root)
      .unwrap_or(path)
      .to_string_lossy()
      .replace('\\', "/");
    let slug = relative.trim_end_matches(".md").to_string();
    slug_map.insert(slug.to_lowercase(), slug.clone());
    slug_map.insert(normalize_slug_key(&slug), slug.clone());
    let basename = file_name_without_md(&relative);
    slug_map.insert(basename.to_lowercase(), slug.clone());
    slug_map.insert(normalize_slug_key(&basename), slug);
  }
  slug_map
}

fn load_page_data(wiki_root: &Path, path: &Path) -> Result<PageData, ProjectRootError> {
  let content = fs::read_to_string(path)?;
  let page = path
    .strip_prefix(wiki_root)
    .unwrap_or(path)
    .to_string_lossy()
    .replace('\\', "/");
  let slug = page.trim_end_matches(".md").to_string();
  let title = extract_title(&content, &page);
  let outlinks = extract_wikilinks(&content);
  // upstream_llm_wiki/src/lib/lint.ts L186-187
  let slug_name = last_path_segment(&slug);
  let window = content.chars().take(SUGGESTION_TOKEN_WINDOW).collect::<String>();
  let tokens = tokenize_for_suggestion(&format!("{title}\n{slug_name}\n{window}"));
  Ok(PageData {
    page,
    slug,
    title,
    outlinks,
    tokens,
  })
}

// upstream_llm_wiki/src/lib/lint.ts L194-205
fn suggest_broken_target<'a>(pages: &'a [PageData], target: &str) -> Option<&'a PageData> {
  let mut best: Option<(&PageData, f64)> = None;
  for candidate in pages {
    let score = string_similarity(target, &candidate.slug)
      .max(string_similarity(target, &candidate.page))
      .max(string_similarity(target, &candidate.title));
    if score > best.map(|(_, existing)| existing).unwrap_or(0.0) {
      best = Some((candidate, score));
    }
  }
  best
    .filter(|(_, score)| *score >= BROKEN_LINK_SUGGESTION_MIN_SCORE)
    .map(|(candidate, _)| candidate)
}

// upstream_llm_wiki/src/lib/lint.ts L207-233
fn suggest_related_page<'a>(
  pages: &'a [PageData],
  page: &PageData,
  direction: SuggestDirection,
) -> Option<&'a PageData> {
  let existing_outlinks = page
    .outlinks
    .iter()
    .map(|link| normalize_link_target(link))
    .collect::<BTreeSet<_>>();
  let mut best: Option<(&PageData, f64)> = None;

  for candidate in pages {
    if candidate.page == page.page {
      continue;
    }
    if direction == SuggestDirection::Target {
      let candidate_keys = [
        normalize_link_target(&candidate.slug),
        normalize_link_target(&candidate.page),
        normalize_link_target(&file_name_without_md(&candidate.page)),
      ];
      if candidate_keys.iter().any(|key| existing_outlinks.contains(key)) {
        continue;
      }
    }

    let mut overlap = 0.0f64;
    for token in &page.tokens {
      if candidate.tokens.contains(token) {
        overlap += if token.chars().count() > 1 {
          1.0
        } else {
          SINGLE_CJK_TOKEN_WEIGHT
        };
      }
    }
    if overlap == 0.0 {
      continue;
    }

    let folder_bonus = if first_path_segment(&page.page) == first_path_segment(&candidate.page) {
      SAME_FOLDER_SCORE_BONUS
    } else {
      0.0
    };
    let score = overlap
      / ((page.tokens.len().max(1) as f64) * (candidate.tokens.len().max(1) as f64)).sqrt()
      + folder_bonus;
    if score > best.map(|(_, existing)| existing).unwrap_or(0.0) {
      best = Some((candidate, score));
    }
  }

  best
    .filter(|(_, score)| *score >= RELATED_PAGE_SUGGESTION_MIN_SCORE)
    .map(|(candidate, _)| candidate)
}

// upstream_llm_wiki/src/lib/lint.ts L57-63
pub fn normalize_link_target(target: &str) -> String {
  let mut value = target.replace('\\', "/");
  if value
    .get(.."wiki/".len())
    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("wiki/"))
  {
    value.drain(.."wiki/".len());
  }
  if value.len() >= ".md".len()
    && value
      .get(value.len() - ".md".len()..)
      .is_some_and(|suffix| suffix.eq_ignore_ascii_case(".md"))
  {
    value.truncate(value.len() - ".md".len());
  }
  value.trim().to_lowercase()
}

// upstream_llm_wiki/src/lib/lint.ts L65-76
fn extract_title(content: &str, fallback_path: &str) -> String {
  static FRONTMATTER_REGEX: OnceLock<Regex> = OnceLock::new();
  static TITLE_REGEX: OnceLock<Regex> = OnceLock::new();
  static HEADING_REGEX: OnceLock<Regex> = OnceLock::new();
  static SEPARATOR_REGEX: OnceLock<Regex> = OnceLock::new();

  let frontmatter_regex =
    FRONTMATTER_REGEX.get_or_init(|| Regex::new(r"\A---\s*\n((?s).*?)\n---").expect("valid regex"));
  if let Some(frontmatter) = frontmatter_regex.captures(content) {
    let title_regex = TITLE_REGEX.get_or_init(|| {
      Regex::new(r#"(?m)^title:\s*["']?(.+?)["']?\s*$"#).expect("valid regex")
    });
    if let Some(captures) = title_regex.captures(frontmatter.get(1).map_or("", |m| m.as_str())) {
      let title = captures.get(1).map_or("", |m| m.as_str()).trim();
      if !title.is_empty() {
        return title.to_string();
      }
    }
  }

  let heading_regex =
    HEADING_REGEX.get_or_init(|| Regex::new(r"(?m)^#\s+(.+)$").expect("valid regex"));
  if let Some(captures) = heading_regex.captures(content) {
    let heading = captures.get(1).map_or("", |m| m.as_str()).trim();
    if !heading.is_empty() {
      return heading.to_string();
    }
  }

  let name = file_name_without_md(fallback_path);
  SEPARATOR_REGEX
    .get_or_init(|| Regex::new(r"[-_]+").expect("valid regex"))
    .replace_all(&name, " ")
    .into_owned()
}

// upstream_llm_wiki/src/lib/lint.ts L78-89
fn tokenize_for_suggestion(text: &str) -> BTreeSet<String> {
  static TOKEN_REGEX: OnceLock<Regex> = OnceLock::new();
  let token_regex =
    TOKEN_REGEX.get_or_init(|| Regex::new(r"[\p{L}\p{N}]+").expect("valid regex"));

  let mut tokens = BTreeSet::new();
  let normalized = text.nfkc().collect::<String>().to_lowercase();
  for found in token_regex.find_iter(&normalized) {
    let token = found.as_str();
    if token.chars().count() >= 2 {
      tokens.insert(token.to_string());
    }
    if token.chars().any(is_cjk_char) {
      for ch in token.chars() {
        tokens.insert(ch.to_string());
      }
    }
  }
  tokens
}

fn is_cjk_char(ch: char) -> bool {
  ('\u{3400}'..='\u{9fff}').contains(&ch)
}

// upstream_llm_wiki/src/lib/lint.ts L91-110
fn levenshtein(a: &str, b: &str) -> usize {
  if a == b {
    return 0;
  }
  let a = a.chars().collect::<Vec<_>>();
  let b = b.chars().collect::<Vec<_>>();
  if a.is_empty() {
    return b.len();
  }
  if b.is_empty() {
    return a.len();
  }

  let mut previous = (0..=b.len()).collect::<Vec<_>>();
  let mut current = vec![0usize; b.len() + 1];
  for i in 1..=a.len() {
    current[0] = i;
    for j in 1..=b.len() {
      let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
      current[j] = (current[j - 1] + 1)
        .min(previous[j] + 1)
        .min(previous[j - 1] + cost);
    }
    previous.copy_from_slice(&current);
  }
  previous[b.len()]
}

// upstream_llm_wiki/src/lib/lint.ts L112-125
fn string_similarity(a: &str, b: &str) -> f64 {
  let left = normalize_link_target(a);
  let right = normalize_link_target(b);
  if left.is_empty() || right.is_empty() {
    return 0.0;
  }
  if left == right {
    return 1.0;
  }
  let left_base = last_path_segment(&left);
  let right_base = last_path_segment(&right);
  if left_base == right_base {
    return SAME_BASENAME_SCORE;
  }
  if right.contains(&left) || left.contains(&right) {
    return CONTAINS_TARGET_SCORE;
  }
  if left_base.chars().count() < 5 || right_base.chars().count() < 5 {
    return 0.0;
  }
  let max_len = left_base.chars().count().max(right_base.chars().count());
  if max_len == 0 {
    return 0.0;
  }
  1.0 - levenshtein(left_base, right_base) as f64 / max_len as f64
}

fn last_path_segment(value: &str) -> &str {
  value.rsplit('/').next().unwrap_or(value)
}

fn first_path_segment(value: &str) -> &str {
  value.split('/').next().unwrap_or(value)
}

fn build_inbound_counts(
  slug_map: &BTreeMap<String, String>,
  pages: &[PageData],
) -> BTreeMap<String, usize> {
  let mut inbound_counts = BTreeMap::new();
  for page in pages {
    for link in &page.outlinks {
      let lookup = link.to_lowercase();
      let normalized = normalize_slug_key(link);
      let target = slug_map
        .get(&lookup)
        .or_else(|| slug_map.get(&normalized))
        .map(|slug| slug.to_lowercase())
        .unwrap_or(lookup);
      *inbound_counts.entry(target).or_insert(0) += 1;
    }
  }
  inbound_counts
}

fn extract_wikilinks(content: &str) -> Vec<String> {
  let chars = content.chars().collect::<Vec<_>>();
  let mut index = 0usize;
  let mut links = Vec::new();

  while index + 1 < chars.len() {
    if chars[index] != '[' || chars[index + 1] != '[' {
      index += 1;
      continue;
    }
    index += 2;
    let mut target = String::new();
    while index + 1 < chars.len() {
      if chars[index] == ']' && chars[index + 1] == ']' {
        break;
      }
      target.push(chars[index]);
      index += 1;
    }
    if index + 1 >= chars.len() {
      break;
    }

    let cleaned = target
      .split('|')
      .next()
      .map(str::trim)
      .filter(|value| !value.is_empty())
      .map(str::to_string);
    if let Some(cleaned) = cleaned {
      links.push(cleaned);
    }
    index += 2;
  }

  links
}

fn file_name_without_md(path: &str) -> String {
  Path::new(path)
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or(path)
    .trim_end_matches(".md")
    .to_string()
}

fn normalize_slug_key(value: &str) -> String {
  value
    .replace('\\', "/")
    .trim_end_matches(".md")
    .to_lowercase()
    .chars()
    .filter(|ch| !matches!(ch, ' ' | '-' | '_'))
    .collect()
}

#[cfg(test)]
mod tests {
  use std::collections::BTreeSet;

  use tempfile::tempdir;

  use crate::project::scaffold::initialize_project;

  use super::{
    CONTAINS_TARGET_SCORE, SAME_BASENAME_SCORE, build_semantic_lint_prompt, levenshtein,
    normalize_link_target, parse_semantic_lint_response, run_structural_lint, string_similarity,
    tokenize_for_suggestion,
  };

  #[test]
  fn structural_lint_reports_orphans_broken_links_and_no_outlinks() {
    let temp = tempdir().unwrap();
    let root = initialize_project(temp.path()).unwrap();

    std::fs::write(
      temp.path().join("wiki/concepts/attention.md"),
      "---\ntype: concept\ntitle: Attention\nsources: []\n---\n\nSee [[missing-page]].\n",
    )
    .unwrap();
    std::fs::write(
      temp.path().join("wiki/concepts/orphan-page.md"),
      "---\ntype: concept\ntitle: Orphan\nsources: []\n---\n\nStandalone.\n",
    )
    .unwrap();

    let result = run_structural_lint(&root).unwrap();
    let summary = result
      .issues
      .iter()
      .map(|issue| format!("{}:{}", issue.issue_type, issue.page))
      .collect::<BTreeSet<_>>();

    assert!(summary.contains("broken-link:concepts/attention.md"));
    assert!(summary.contains("orphan:concepts/attention.md"));
    assert!(summary.contains("orphan:concepts/orphan-page.md"));
    assert!(summary.contains("no-outlinks:concepts/orphan-page.md"));
  }

  #[test]
  fn structural_lint_matches_links_case_insensitively() {
    let temp = tempdir().unwrap();
    let root = initialize_project(temp.path()).unwrap();

    std::fs::write(
      temp.path().join("wiki/concepts/attention.md"),
      "---\ntype: concept\ntitle: Attention\nsources: []\n---\n\nSee [[Reasoning Models]].\n",
    )
    .unwrap();
    std::fs::write(
      temp.path().join("wiki/entities/reasoning-models.md"),
      "---\ntype: entity\ntitle: Reasoning Models\nsources: []\n---\n\n# Reasoning Models\n",
    )
    .unwrap();

    let result = run_structural_lint(&root).unwrap();
    assert!(
      !result
        .issues
        .iter()
        .any(|issue| issue.issue_type == "broken-link")
    );
  }

  #[test]
  fn string_similarity_matches_upstream_boundaries() {
    // upstream_llm_wiki/src/lib/lint.ts L112-125
    assert_eq!(normalize_link_target("Wiki/Entities/Foo.MD"), "entities/foo");
    assert_eq!(string_similarity("wiki/entities/foo.md", "entities/foo"), 1.0);
    assert_eq!(
      string_similarity("concepts/attention", "entities/attention"),
      SAME_BASENAME_SCORE
    );
    assert_eq!(
      string_similarity("attention", "concepts/attention-mechanism"),
      CONTAINS_TARGET_SCORE
    );
    // basenames shorter than 5 chars never fuzzy-match
    assert_eq!(string_similarity("abc", "abd"), 0.0);
    // levenshtein path: 1 edit over max length 16
    assert_eq!(levenshtein("reasoning-modals", "reasoning-models"), 1);
    let score = string_similarity("queries/reasoning-modals", "entities/reasoning-models");
    assert!((score - (1.0 - 1.0 / 16.0)).abs() < 1e-9);
  }

  #[test]
  fn tokenize_for_suggestion_handles_cjk_and_short_tokens() {
    // upstream_llm_wiki/src/lib/lint.ts L78-89
    let tokens = tokenize_for_suggestion("Attention 注意力 a b2");
    assert!(tokens.contains("attention"));
    assert!(tokens.contains("注意力"));
    assert!(tokens.contains("注"));
    assert!(tokens.contains("意"));
    assert!(tokens.contains("力"));
    assert!(tokens.contains("b2"));
    assert!(!tokens.contains("a"));
  }

  #[test]
  fn structural_lint_attaches_fix_suggestions() {
    let temp = tempdir().unwrap();
    let root = initialize_project(temp.path()).unwrap();

    std::fs::write(
      temp.path().join("wiki/concepts/attention.md"),
      "---\ntype: concept\ntitle: Attention\nsources: []\n---\n\nAttention mechanism scores tokens. See [[attention-mechanisms]].\n",
    )
    .unwrap();
    std::fs::write(
      temp.path().join("wiki/concepts/attention-mechanism.md"),
      "---\ntype: concept\ntitle: Attention Mechanism\nsources: []\n---\n\nAttention mechanism scores tokens across positions.\n",
    )
    .unwrap();

    let result = run_structural_lint(&root).unwrap();

    let broken = result
      .issues
      .iter()
      .find(|issue| issue.issue_type == "broken-link")
      .expect("broken-link issue");
    assert_eq!(broken.broken_target.as_deref(), Some("attention-mechanisms"));
    assert_eq!(
      broken.suggested_target.as_deref(),
      Some("concepts/attention-mechanism.md")
    );

    let orphan = result
      .issues
      .iter()
      .find(|issue| issue.issue_type == "orphan" && issue.page == "concepts/attention.md")
      .expect("orphan issue");
    assert_eq!(
      orphan.suggested_source.as_deref(),
      Some("concepts/attention-mechanism.md")
    );

    let no_outlinks = result
      .issues
      .iter()
      .find(|issue| issue.issue_type == "no-outlinks")
      .expect("no-outlinks issue");
    assert_eq!(no_outlinks.page, "concepts/attention-mechanism.md");
    assert_eq!(
      no_outlinks.suggested_target.as_deref(),
      Some("concepts/attention.md")
    );
  }

  #[test]
  fn semantic_lint_prompt_summarizes_wiki_pages() {
    let temp = tempdir().unwrap();
    let root = initialize_project(temp.path()).unwrap();

    std::fs::write(
      temp.path().join("wiki/concepts/attention.md"),
      "---\ntype: concept\ntitle: Attention\nsources: []\n---\n\n# Attention\n\nAttention focuses computation.\n",
    )
    .unwrap();

    let prompt = build_semantic_lint_prompt(&root).unwrap().unwrap();
    assert!(prompt.contains("### concepts/attention.md"));
    assert!(prompt.contains("Attention focuses computation."));
  }

  #[test]
  fn parse_semantic_lint_response_extracts_blocks() {
    let result = parse_semantic_lint_response(
      "---LINT: contradiction | warning | Conflicting attention claims---\nTwo pages disagree.\nPAGES: concepts/attention.md, concepts/attention-mechanism.md\n---END LINT---",
    );

    assert_eq!(result.mode, "semantic");
    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].issue_type, "semantic");
    assert_eq!(result.issues[0].severity, "warning");
    assert_eq!(result.issues[0].page, "Conflicting attention claims");
    assert_eq!(
      result.issues[0].detail,
      "[contradiction] Two pages disagree."
    );
    assert_eq!(
      result.issues[0].affected_pages,
      vec!["concepts/attention.md".to_string(), "concepts/attention-mechanism.md".to_string()]
    );
  }
}
