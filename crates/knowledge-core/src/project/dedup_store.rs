use std::fs;

use serde::{Deserialize, Serialize};

use super::dedup::normalize_group_key;
use super::root::{ProjectRoot, ProjectRootError};

const GROUPS_FILE: &str = ".knowledge/dedup/groups.json";
const NOT_DUPLICATES_FILE: &str = ".knowledge/dedup/not-duplicates.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DedupGroup {
  pub id: String,
  pub slugs: Vec<String>,
  pub reason: String,
  pub confidence: String,
  pub status: String,
  pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DedupStore {
  pub groups: Vec<DedupGroup>,
}

pub fn load_dedup_store(root: &ProjectRoot) -> Result<DedupStore, ProjectRootError> {
  let path = root.safe_join(GROUPS_FILE)?;
  if !path.exists() {
    return Ok(DedupStore::default());
  }
  let raw = fs::read_to_string(path)?;
  serde_json::from_str(&raw).map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))
}

pub fn save_dedup_store(root: &ProjectRoot, store: &DedupStore) -> Result<(), ProjectRootError> {
  let path = root.safe_join(GROUPS_FILE)?;
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  let raw = serde_json::to_string_pretty(store)
    .map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))?;
  fs::write(path, raw)?;
  Ok(())
}

/// Ported from dedup-storage.ts loadNotDuplicates — missing or garbage file → [].
pub fn load_not_duplicates(root: &ProjectRoot) -> Result<Vec<Vec<String>>, ProjectRootError> {
  let path = root.safe_join(NOT_DUPLICATES_FILE)?;
  if !path.exists() {
    return Ok(Vec::new());
  }
  let raw = fs::read_to_string(path)?;
  Ok(serde_json::from_str(&raw).unwrap_or_default())
}

/// Ported from dedup-storage.ts addNotDuplicate — idempotent, sorted, <2 slugs no-op.
pub fn add_not_duplicate(root: &ProjectRoot, slugs: &[String]) -> Result<(), ProjectRootError> {
  if slugs.len() < 2 {
    return Ok(());
  }
  let mut groups = load_not_duplicates(root)?;
  let key = normalize_group_key(slugs);
  if groups.iter().any(|group| normalize_group_key(group) == key) {
    return Ok(());
  }
  let mut sorted = slugs.to_vec();
  sorted.sort();
  groups.push(sorted);

  let path = root.safe_join(NOT_DUPLICATES_FILE)?;
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  let raw = serde_json::to_string_pretty(&groups)
    .map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))?;
  fs::write(path, raw)?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::project::root::ProjectRoot;

  #[test]
  fn load_dedup_store_defaults_when_missing_and_errors_on_garbage() {
    let temp = tempfile::tempdir().unwrap();
    let root = ProjectRoot::new(temp.path()).unwrap();
    assert!(load_dedup_store(&root).unwrap().groups.is_empty());

    std::fs::create_dir_all(temp.path().join(".knowledge/dedup")).unwrap();
    std::fs::write(temp.path().join(".knowledge/dedup/groups.json"), "not json").unwrap();
    assert!(load_dedup_store(&root).is_err());
  }

  #[test]
  fn save_and_load_dedup_store_round_trips() {
    let temp = tempfile::tempdir().unwrap();
    let root = ProjectRoot::new(temp.path()).unwrap();
    let store = DedupStore {
      groups: vec![DedupGroup {
        id: "group-1".to_string(),
        slugs: vec!["a".to_string(), "b".to_string()],
        reason: "same thing".to_string(),
        confidence: "high".to_string(),
        status: "candidate".to_string(),
        created_at: "2026-06-12T00:00:00Z".to_string(),
      }],
    };
    save_dedup_store(&root, &store).unwrap();
    let loaded = load_dedup_store(&root).unwrap();
    assert_eq!(loaded.groups, store.groups);
    let raw = std::fs::read_to_string(temp.path().join(".knowledge/dedup/groups.json")).unwrap();
    assert!(raw.contains("\"createdAt\""));
  }

  #[test]
  fn not_duplicates_load_is_tolerant_and_add_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let root = ProjectRoot::new(temp.path()).unwrap();
    assert!(load_not_duplicates(&root).unwrap().is_empty());

    std::fs::create_dir_all(temp.path().join(".knowledge/dedup")).unwrap();
    std::fs::write(temp.path().join(".knowledge/dedup/not-duplicates.json"), "garbage").unwrap();
    assert!(load_not_duplicates(&root).unwrap().is_empty());

    add_not_duplicate(&root, &["b".to_string(), "a".to_string()]).unwrap();
    add_not_duplicate(&root, &["A".to_string(), "B".to_string()]).unwrap();
    add_not_duplicate(&root, &["only-one".to_string()]).unwrap();
    let groups = load_not_duplicates(&root).unwrap();
    assert_eq!(groups, vec![vec!["a".to_string(), "b".to_string()]]);
  }
}
