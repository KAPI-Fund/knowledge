use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::project::root::{ProjectRoot, ProjectRootError};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StructuralLintIssue {
  pub issue_type: String,
  pub severity: String,
  pub page: String,
  pub detail: String,
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
  outlinks: Vec<String>,
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
      issues.push(StructuralLintIssue {
        issue_type: "orphan".to_string(),
        severity: "info".to_string(),
        page: page.page.clone(),
        detail: "No other pages link to this page.".to_string(),
      });
    }

    if page.outlinks.is_empty() {
      issues.push(StructuralLintIssue {
        issue_type: "no-outlinks".to_string(),
        severity: "info".to_string(),
        page: page.page.clone(),
        detail: "This page has no [[wikilink]] references to other pages.".to_string(),
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

      issues.push(StructuralLintIssue {
        issue_type: "broken-link".to_string(),
        severity: "warning".to_string(),
        page: page.page.clone(),
        detail: format!("Broken link: [[{link}]] - target page not found."),
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
  let outlinks = extract_wikilinks(&content);
  Ok(PageData {
    page,
    slug,
    outlinks,
  })
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

  use super::{build_semantic_lint_prompt, parse_semantic_lint_response, run_structural_lint};

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
