use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::project::root::{ProjectRoot, ProjectRootError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItem {
  pub id: String,
  pub status: String,
  pub title: String,
  pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReviewStore {
  pub reviews: Vec<ReviewItem>,
}

pub fn load_reviews(root: &ProjectRoot) -> Result<ReviewStore, ProjectRootError> {
  let path = root.safe_join(".knowledge/reviews/items.json")?;
  let raw = fs::read_to_string(path)?;
  serde_json::from_str(&raw).map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))
}

pub fn save_reviews(root: &ProjectRoot, store: &ReviewStore) -> Result<(), ProjectRootError> {
  let path = root.safe_join(".knowledge/reviews/items.json")?;
  let json = serde_json::to_string(store)
    .map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))?;
  fs::write(path, json)?;
  Ok(())
}

pub fn maybe_add_review_for_source(
  root: &ProjectRoot,
  source_name: &str,
  content: &str,
) -> Result<(), ProjectRootError> {
  if !content.to_lowercase().contains("missing") && !content.to_lowercase().contains("question") {
    return Ok(());
  }

  let mut store = load_reviews(root)?;
  let id = format!("review-{}", source_name.trim_end_matches(".md"));
  if store.reviews.iter().any(|review| review.id == id) {
    return Ok(());
  }

  store.reviews.push(ReviewItem {
    id,
    status: "open".to_string(),
    title: format!("Review {source_name}"),
    description: "Potential unresolved knowledge gap detected during ingest.".to_string(),
  });
  save_reviews(root, &store)
}

pub fn update_review_status(
  root: &ProjectRoot,
  review_id: &str,
  status: &str,
) -> Result<ReviewItem, ProjectRootError> {
  let mut store = load_reviews(root)?;
  let review = store
    .reviews
    .iter_mut()
    .find(|review| review.id == review_id)
    .ok_or_else(|| ProjectRootError::InvalidSourcePayload("review not found".to_string()))?;
  review.status = status.to_string();
  let updated = review.clone();
  save_reviews(root, &store)?;
  Ok(updated)
}

pub fn _review_path(root: &Path) -> std::path::PathBuf {
  root.join(".knowledge/reviews/items.json")
}
