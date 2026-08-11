//! Sidecar persistence for lint items at .knowledge/lint/items.json,
//! mirroring the reviews sidecar pattern (reviews.rs). Unlike reviews,
//! a missing file yields an empty store.

use std::collections::BTreeSet;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::project::root::{ProjectRoot, ProjectRootError};

const LINT_ITEMS_REL: &str = ".knowledge/lint/items.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LintItem {
  pub id: String,
  pub issue_type: String,
  pub severity: String,
  pub page: String,
  pub detail: String,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub affected_pages: Vec<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub broken_target: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub suggested_target: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub suggested_source: Option<String>,
  pub mode: String,
  pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LintItemStore {
  pub items: Vec<LintItem>,
}

pub fn load_lint_items(root: &ProjectRoot) -> Result<LintItemStore, ProjectRootError> {
  let path = root.safe_join(LINT_ITEMS_REL)?;
  let raw = match fs::read_to_string(path) {
    Ok(raw) => raw,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
      return Ok(LintItemStore::default());
    }
    Err(error) => return Err(error.into()),
  };
  serde_json::from_str(&raw)
    .map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))
}

pub fn save_lint_items(root: &ProjectRoot, store: &LintItemStore) -> Result<(), ProjectRootError> {
  let path = root.safe_join(LINT_ITEMS_REL)?;
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  let json = serde_json::to_string(store)
    .map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))?;
  fs::write(path, json)?;
  Ok(())
}

/// Replace all items produced by one lint mode (structural / semantic)
/// while leaving the other mode's items untouched.
pub fn replace_items_for_mode(
  root: &ProjectRoot,
  mode: &str,
  items: Vec<LintItem>,
) -> Result<usize, ProjectRootError> {
  let mut store = load_lint_items(root)?;
  store.items.retain(|item| item.mode != mode);
  let inserted = items.len();
  store.items.extend(items);
  save_lint_items(root, &store)?;
  Ok(inserted)
}

/// Remove items by id, returning the removed items.
pub fn remove_lint_items(
  root: &ProjectRoot,
  ids: &[String],
) -> Result<Vec<LintItem>, ProjectRootError> {
  let mut store = load_lint_items(root)?;
  let wanted = ids.iter().collect::<BTreeSet<_>>();
  let mut removed = Vec::new();
  store.items.retain(|item| {
    if wanted.contains(&item.id) {
      removed.push(item.clone());
      false
    } else {
      true
    }
  });
  if !removed.is_empty() {
    save_lint_items(root, &store)?;
  }
  Ok(removed)
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use crate::project::scaffold::initialize_project;

  use super::{LintItem, load_lint_items, remove_lint_items, replace_items_for_mode};

  fn item(id: &str, mode: &str) -> LintItem {
    LintItem {
      id: id.to_string(),
      issue_type: "orphan".to_string(),
      severity: "info".to_string(),
      page: "concepts/example.md".to_string(),
      detail: "No other pages link to this page.".to_string(),
      affected_pages: Vec::new(),
      broken_target: None,
      suggested_target: None,
      suggested_source: None,
      mode: mode.to_string(),
      created_at: "2026-07-11T00:00:00Z".to_string(),
    }
  }

  #[test]
  fn load_returns_empty_store_when_file_missing() {
    let temp = tempdir().unwrap();
    let root = initialize_project(temp.path()).unwrap();
    let store = load_lint_items(&root).unwrap();
    assert!(store.items.is_empty());
  }

  #[test]
  fn replace_items_for_mode_keeps_other_mode() {
    let temp = tempdir().unwrap();
    let root = initialize_project(temp.path()).unwrap();

    replace_items_for_mode(&root, "structural", vec![item("a", "structural")]).unwrap();
    replace_items_for_mode(&root, "semantic", vec![item("b", "semantic")]).unwrap();
    replace_items_for_mode(&root, "structural", vec![item("c", "structural")]).unwrap();

    let store = load_lint_items(&root).unwrap();
    let ids = store.items.iter().map(|item| item.id.as_str()).collect::<Vec<_>>();
    assert_eq!(ids, vec!["b", "c"]);
  }

  #[test]
  fn remove_lint_items_returns_removed_entries() {
    let temp = tempdir().unwrap();
    let root = initialize_project(temp.path()).unwrap();

    replace_items_for_mode(
      &root,
      "structural",
      vec![item("a", "structural"), item("b", "structural")],
    )
    .unwrap();

    let removed = remove_lint_items(&root, &["a".to_string(), "missing".to_string()]).unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].id, "a");

    let store = load_lint_items(&root).unwrap();
    assert_eq!(store.items.len(), 1);
    assert_eq!(store.items[0].id, "b");
  }
}
