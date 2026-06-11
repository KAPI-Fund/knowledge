//! Wiki page write operations: save (create/update) and cascade
//! delete with reference cleanup. Delete semantics ported from
//! upstream_llm_wiki/src/lib/wiki-page-delete.ts.

use std::fs;
use std::path::{Path, PathBuf};

use crate::ingest::{parse_frontmatter_array, write_frontmatter_array};
use crate::project::files::{MAX_FILE_CONTENT_BYTES, is_public_project_rel};
use crate::project::root::{ProjectRoot, ProjectRootError};
use crate::project::wiki_cleanup::{
  DeletedPageInfo, build_deleted_keys, clean_index_listing, extract_frontmatter_title,
  normalize_wiki_ref_key, strip_deleted_wikilinks,
};

#[derive(Debug, thiserror::Error)]
pub enum WikiPageError {
  #[error("path must be a markdown file under wiki/")]
  NotWikiPage,
  #[error("file is too large to save via API")]
  FileTooLarge,
  #[error("io error: {0}")]
  Io(#[from] std::io::Error),
  #[error("{0}")]
  Root(#[from] ProjectRootError),
}

#[derive(Debug, Clone)]
pub struct SavedWikiPage {
  pub path: String,
  pub created: bool,
}

pub fn is_wiki_page_rel(relative_path: &str) -> bool {
  let normalized = normalize_rel(relative_path);
  let lower = normalized.to_lowercase();
  lower.starts_with("wiki/") && lower.ends_with(".md") && is_public_project_rel(&normalized)
}

fn normalize_rel(value: &str) -> String {
  value
    .replace('\\', "/")
    .trim_start_matches('/')
    .trim_end_matches('/')
    .to_string()
}

pub fn save_wiki_page(
  root: &ProjectRoot,
  relative_path: &str,
  content: &str,
) -> Result<SavedWikiPage, WikiPageError> {
  let normalized = normalize_rel(relative_path);
  if !is_wiki_page_rel(&normalized) {
    return Err(WikiPageError::NotWikiPage);
  }
  if content.len() as u64 > MAX_FILE_CONTENT_BYTES {
    return Err(WikiPageError::FileTooLarge);
  }

  let path = root.safe_join(&normalized)?;
  let created = !path.exists();
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  fs::write(&path, content)?;

  Ok(SavedWikiPage {
    path: normalized,
    created,
  })
}

#[derive(Debug, Clone)]
pub struct WikiPagesDeleteResult {
  pub deleted_paths: Vec<String>,
  pub deleted_page_ids: Vec<String>,
  pub rewritten_files: usize,
}

pub fn delete_wiki_pages_with_refs(
  root: &ProjectRoot,
  relative_paths: &[String],
) -> Result<WikiPagesDeleteResult, WikiPageError> {
  let mut result = WikiPagesDeleteResult {
    deleted_paths: Vec::new(),
    deleted_page_ids: Vec::new(),
    rewritten_files: 0,
  };

  // 1. Validate the whole batch and snapshot slug + title before
  //    deleting anything (titles are unreadable after the delete).
  let mut targets = Vec::new();
  let mut infos = Vec::new();
  for relative_path in relative_paths {
    let normalized = normalize_rel(relative_path);
    if !is_wiki_page_rel(&normalized) {
      return Err(WikiPageError::NotWikiPage);
    }
    let absolute = root.safe_join(&normalized)?;
    let slug = file_stem(&normalized);
    let title = fs::read_to_string(&absolute)
      .map(|content| extract_frontmatter_title(&content))
      .unwrap_or_default();
    if !slug.is_empty() {
      infos.push(DeletedPageInfo {
        slug: slug.clone(),
        title,
      });
    }
    targets.push((normalized, absolute, slug));
  }

  // 2. Delete each page; cascade the media directory for source pages.
  for (normalized, absolute, slug) in &targets {
    match fs::remove_file(absolute) {
      Ok(()) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
      Err(error) => return Err(WikiPageError::Io(error)),
    }
    result.deleted_paths.push(normalized.clone());
    if !slug.is_empty() {
      result.deleted_page_ids.push(slug.clone());
    }

    // Source pages own wiki/media/<slug>/. Guard the slug so a
    // degenerate name can never resolve to the media root itself.
    if normalized.to_lowercase().starts_with("wiki/sources/")
      && !slug.is_empty()
      && !slug.starts_with('.')
      && let Ok(media_dir) = root.safe_join(&format!("wiki/media/{slug}"))
    {
      let _ = fs::remove_dir_all(media_dir);
    }
  }

  if result.deleted_paths.is_empty() {
    return Ok(result);
  }

  // 3. Best-effort sweep of surviving wiki markdown files.
  let deleted_keys = build_deleted_keys(&infos);
  let wiki_root = root.safe_join("wiki")?;
  let mut markdown_files = Vec::new();
  collect_markdown_files(&wiki_root, &mut markdown_files);

  for path in markdown_files {
    let Ok(content) = fs::read_to_string(&path) else {
      continue;
    };

    let mut updated = content.clone();
    if path.file_name().and_then(|value| value.to_str()) == Some("index.md") {
      updated = clean_index_listing(&updated, &deleted_keys);
    }
    updated = strip_deleted_wikilinks(&updated, &deleted_keys);

    let related = parse_frontmatter_array(&updated, "related");
    if !related.is_empty() {
      let filtered = related
        .iter()
        .filter(|value| !deleted_keys.contains(&normalize_wiki_ref_key(value)))
        .cloned()
        .collect::<Vec<_>>();
      if filtered.len() != related.len() {
        updated = write_frontmatter_array(&updated, "related", &filtered);
      }
    }

    if updated != content && fs::write(&path, updated).is_ok() {
      result.rewritten_files += 1;
    }
  }

  Ok(result)
}

fn file_stem(relative_path: &str) -> String {
  Path::new(relative_path)
    .file_stem()
    .and_then(|value| value.to_str())
    .unwrap_or_default()
    .to_string()
}

fn collect_markdown_files(dir: &Path, output: &mut Vec<PathBuf>) {
  let Ok(entries) = fs::read_dir(dir) else {
    return;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    let Ok(file_type) = entry.file_type() else {
      continue;
    };
    if file_type.is_dir() {
      collect_markdown_files(&path, output);
      continue;
    }
    if path.extension().and_then(|value| value.to_str()) == Some("md") {
      output.push(path);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::project::root::ProjectRoot;
  use std::fs;

  fn project_root() -> (tempfile::TempDir, ProjectRoot) {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("wiki/concepts")).unwrap();
    let root = ProjectRoot::new(temp.path()).unwrap();
    (temp, root)
  }

  #[test]
  fn save_creates_then_updates_wiki_page() {
    let (_temp, root) = project_root();

    let created = save_wiki_page(&root, "wiki/concepts/kv-cache.md", "# KV Cache\n").unwrap();
    assert_eq!(created.path, "wiki/concepts/kv-cache.md");
    assert!(created.created);

    let updated = save_wiki_page(&root, "wiki/concepts/kv-cache.md", "# KV Cache v2\n").unwrap();
    assert!(!updated.created);
    let content = fs::read_to_string(root.safe_join("wiki/concepts/kv-cache.md").unwrap()).unwrap();
    assert_eq!(content, "# KV Cache v2\n");
  }

  #[test]
  fn save_creates_missing_parent_directories() {
    let (_temp, root) = project_root();
    save_wiki_page(&root, "wiki/entities/new-dir/page.md", "body").unwrap();
    assert!(root.safe_join("wiki/entities/new-dir/page.md").unwrap().exists());
  }

  #[test]
  fn save_rejects_non_wiki_paths() {
    let (_temp, root) = project_root();
    assert!(matches!(
      save_wiki_page(&root, "raw/sources/notes.md", "x"),
      Err(WikiPageError::NotWikiPage)
    ));
    assert!(matches!(
      save_wiki_page(&root, "wiki/page.txt", "x"),
      Err(WikiPageError::NotWikiPage)
    ));
    assert!(matches!(
      save_wiki_page(&root, "wiki/../escape.md", "x"),
      Err(WikiPageError::NotWikiPage)
    ));
    assert!(matches!(
      save_wiki_page(&root, "wiki/.hidden.md", "x"),
      Err(WikiPageError::NotWikiPage)
    ));
  }

  #[test]
  fn cascade_delete_cleans_index_body_links_related_and_media() {
    let (_temp, root) = project_root();
    fs::create_dir_all(root.safe_join("wiki/sources").unwrap()).unwrap();
    fs::create_dir_all(root.safe_join("wiki/media/paper").unwrap()).unwrap();
    fs::write(root.safe_join("wiki/media/paper/fig1.png").unwrap(), [0u8]).unwrap();

    fs::write(
      root.safe_join("wiki/sources/paper.md").unwrap(),
      "---\ntype: source\ntitle: The Paper\n---\n\n# The Paper\n",
    )
    .unwrap();
    fs::write(
      root.safe_join("wiki/concepts/kv-cache.md").unwrap(),
      "---\ntype: concept\ntitle: KV Cache\n---\n\n# KV Cache\n",
    )
    .unwrap();
    fs::write(
      root.safe_join("wiki/concepts/attention.md").unwrap(),
      "---\ntype: concept\ntitle: Attention\nrelated: [\"kv-cache\", \"transformer\"]\n---\n\nUses [[KV Cache]] and [[Transformer]].\n",
    )
    .unwrap();
    fs::write(
      root.safe_join("wiki/index.md").unwrap(),
      "# Index\n\n- [[KV Cache]] cached states\n- [[The Paper]] source\n- [[Attention]] focus\n",
    )
    .unwrap();

    let result = delete_wiki_pages_with_refs(
      &root,
      &[
        "wiki/concepts/kv-cache.md".to_string(),
        "wiki/sources/paper.md".to_string(),
      ],
    )
    .unwrap();

    assert_eq!(result.deleted_paths.len(), 2);
    assert!(result.deleted_page_ids.contains(&"kv-cache".to_string()));
    assert!(result.deleted_page_ids.contains(&"paper".to_string()));
    assert_eq!(result.rewritten_files, 2);

    assert!(!root.safe_join("wiki/concepts/kv-cache.md").unwrap().exists());
    assert!(!root.safe_join("wiki/media/paper").unwrap().exists());

    let index = fs::read_to_string(root.safe_join("wiki/index.md").unwrap()).unwrap();
    assert!(!index.contains("KV Cache"));
    assert!(!index.contains("The Paper"));
    assert!(index.contains("- [[Attention]] focus"));

    let attention = fs::read_to_string(root.safe_join("wiki/concepts/attention.md").unwrap()).unwrap();
    assert!(attention.contains("Uses KV Cache and [[Transformer]]."));
    assert!(attention.contains("related: [\"transformer\"]"));
  }

  #[test]
  fn cascade_delete_skips_missing_files_without_error() {
    let (_temp, root) = project_root();
    let result =
      delete_wiki_pages_with_refs(&root, &["wiki/concepts/ghost.md".to_string()]).unwrap();
    assert!(result.deleted_paths.is_empty());
    assert_eq!(result.rewritten_files, 0);
  }
}
