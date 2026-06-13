use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::page_merge::{parse_frontmatter_array, split_frontmatter_lines_owned};
use super::root::{ProjectRoot, ProjectRootError};

/// Ported from upstream_llm_wiki/src/lib/dedup.ts (EntitySummary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySummary {
  pub slug: String,
  pub path: String,
  pub page_type: String,
  pub title: String,
  pub description: Option<String>,
  pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupPage {
  pub slug: String,
  pub path: String,
  pub content: String,
}

pub fn extract_entity_summary(relative_path: &str, content: &str) -> Option<EntitySummary> {
  let (frontmatter_lines, body, _newline) = split_frontmatter_lines_owned(content)?;

  let mut fields = BTreeMap::new();
  for line in &frontmatter_lines {
    let Some((key, value)) = line.split_once(':') else {
      continue;
    };
    let normalized = value.trim().trim_matches('"').trim_matches('\'').trim();
    if normalized.is_empty() {
      continue;
    }
    fields.insert(key.trim().to_string(), normalized.to_string());
  }

  let slug = slug_from_path(relative_path);
  let page_type = fields
    .get("type")
    .cloned()
    .unwrap_or_else(|| "unknown".to_string());
  let title = fields.get("title").cloned().unwrap_or_else(|| slug.clone());
  let description = fields
    .get("description")
    .cloned()
    .or_else(|| first_body_paragraph(&body))
    .map(|value| truncate_chars(&value, 200));
  let tags = parse_frontmatter_array(content, "tags");

  Some(EntitySummary {
    slug,
    path: relative_path.to_string(),
    page_type,
    title,
    description,
    tags,
  })
}

pub fn slug_from_path(path: &str) -> String {
  let base = path.rsplit('/').next().unwrap_or(path);
  base.strip_suffix(".md").unwrap_or(base).to_string()
}

fn first_body_paragraph(body: &str) -> Option<String> {
  body
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .find(|line| !line.starts_with('#') && !line.starts_with('|'))
    .map(str::to_string)
}

fn truncate_chars(value: &str, max: usize) -> String {
  if value.chars().count() <= max {
    return value.to_string();
  }
  let mut truncated = value.chars().take(max - 1).collect::<String>();
  truncated.push('…');
  truncated
}

/// Walk wiki/entities + wiki/concepts (upstream's extract scope), sorted by path.
pub fn collect_entity_pages(root: &ProjectRoot) -> Result<Vec<DedupPage>, ProjectRootError> {
  let mut pages = Vec::new();
  for subdir in ["wiki/concepts", "wiki/entities"] {
    let dir = root.safe_join(subdir)?;
    if dir.exists() {
      collect_markdown_pages(root.as_path(), &dir, &mut pages)?;
    }
  }
  pages.sort_by(|left, right| left.path.cmp(&right.path));
  Ok(pages)
}

/// Every .md under wiki/ — upstream's MergeRequest.otherWikiPages source.
pub fn collect_all_wiki_pages(root: &ProjectRoot) -> Result<Vec<DedupPage>, ProjectRootError> {
  let mut pages = Vec::new();
  let wiki_root = root.safe_join("wiki")?;
  if wiki_root.exists() {
    collect_markdown_pages(root.as_path(), &wiki_root, &mut pages)?;
  }
  pages.sort_by(|left, right| left.path.cmp(&right.path));
  Ok(pages)
}

fn collect_markdown_pages(
  project_root: &Path,
  dir: &Path,
  pages: &mut Vec<DedupPage>,
) -> Result<(), ProjectRootError> {
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      collect_markdown_pages(project_root, &path, pages)?;
      continue;
    }
    if path.extension().and_then(|value| value.to_str()) != Some("md") {
      continue;
    }
    let relative_path = path
      .strip_prefix(project_root)
      .unwrap_or(&path)
      .to_string_lossy()
      .replace('\\', "/");
    let content = fs::read_to_string(&path)?;
    pages.push(DedupPage {
      slug: slug_from_path(&relative_path),
      path: relative_path,
      content,
    });
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extract_entity_summary_reads_frontmatter_fields() {
    let content = "---\ntype: entity\ntitle: Volatile Fatty Acids\ndescription: Short-chain fatty acids.\ntags: [chemistry, metabolism]\n---\n\n# VFA\n\nBody text here.\n";
    let summary = extract_entity_summary("wiki/entities/vfa.md", content).unwrap();
    assert_eq!(summary.slug, "vfa");
    assert_eq!(summary.path, "wiki/entities/vfa.md");
    assert_eq!(summary.page_type, "entity");
    assert_eq!(summary.title, "Volatile Fatty Acids");
    assert_eq!(summary.description.as_deref(), Some("Short-chain fatty acids."));
    assert_eq!(summary.tags, vec!["chemistry".to_string(), "metabolism".to_string()]);
  }

  #[test]
  fn extract_entity_summary_returns_none_without_frontmatter() {
    assert!(extract_entity_summary("wiki/entities/foo.md", "# Foo\n\nNo frontmatter.").is_none());
  }

  #[test]
  fn extract_entity_summary_falls_back_to_first_body_paragraph() {
    let content = "---\ntype: concept\ntitle: Attention\n---\n\n# Attention\n\n| a | b |\n\nFocuses computation on relevant tokens.\n";
    let summary = extract_entity_summary("wiki/concepts/attention.md", content).unwrap();
    assert_eq!(
      summary.description.as_deref(),
      Some("Focuses computation on relevant tokens.")
    );
  }

  #[test]
  fn extract_entity_summary_truncates_long_descriptions() {
    let long_line = "x".repeat(300);
    let content = format!("---\ntype: concept\ntitle: Foo\n---\n\n{long_line}\n");
    let summary = extract_entity_summary("wiki/concepts/foo.md", &content).unwrap();
    let description = summary.description.unwrap();
    assert_eq!(description.chars().count(), 200);
    assert!(description.ends_with('…'));
  }

  #[test]
  fn extract_entity_summary_defaults_title_and_type() {
    let content = "---\ncreated: 2026-06-12\n---\n\nBody.\n";
    let summary = extract_entity_summary("wiki/entities/some-slug.md", content).unwrap();
    assert_eq!(summary.title, "some-slug");
    assert_eq!(summary.page_type, "unknown");
    assert!(summary.tags.is_empty());
  }

  #[test]
  fn collect_entity_pages_walks_entities_and_concepts_only() {
    let temp = tempfile::tempdir().unwrap();
    let root = crate::project::root::ProjectRoot::new(temp.path()).unwrap();
    std::fs::create_dir_all(temp.path().join("wiki/entities")).unwrap();
    std::fs::create_dir_all(temp.path().join("wiki/concepts/nested")).unwrap();
    std::fs::create_dir_all(temp.path().join("wiki/sources")).unwrap();
    std::fs::write(temp.path().join("wiki/entities/b.md"), "b").unwrap();
    std::fs::write(temp.path().join("wiki/concepts/a.md"), "a").unwrap();
    std::fs::write(temp.path().join("wiki/concepts/nested/c.md"), "c").unwrap();
    std::fs::write(temp.path().join("wiki/concepts/skip.txt"), "no").unwrap();
    std::fs::write(temp.path().join("wiki/sources/s.md"), "s").unwrap();

    let pages = collect_entity_pages(&root).unwrap();
    let paths = pages.iter().map(|page| page.path.as_str()).collect::<Vec<_>>();
    assert_eq!(
      paths,
      vec!["wiki/concepts/a.md", "wiki/concepts/nested/c.md", "wiki/entities/b.md"]
    );
    assert_eq!(pages[0].slug, "a");
    assert_eq!(pages[0].content, "a");
  }

  #[test]
  fn collect_all_wiki_pages_includes_every_markdown_file() {
    let temp = tempfile::tempdir().unwrap();
    let root = crate::project::root::ProjectRoot::new(temp.path()).unwrap();
    std::fs::create_dir_all(temp.path().join("wiki/sources")).unwrap();
    std::fs::write(temp.path().join("wiki/index.md"), "index").unwrap();
    std::fs::write(temp.path().join("wiki/sources/s.md"), "s").unwrap();

    let pages = collect_all_wiki_pages(&root).unwrap();
    let paths = pages.iter().map(|page| page.path.as_str()).collect::<Vec<_>>();
    assert_eq!(paths, vec!["wiki/index.md", "wiki/sources/s.md"]);
  }
}
