//! Wiki page write operations: save (create/update) and cascade
//! delete with reference cleanup. Delete semantics ported from
//! upstream_llm_wiki/src/lib/wiki-page-delete.ts.

use std::fs;

use crate::project::files::{MAX_FILE_CONTENT_BYTES, is_public_project_rel};
use crate::project::root::{ProjectRoot, ProjectRootError};

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
}
