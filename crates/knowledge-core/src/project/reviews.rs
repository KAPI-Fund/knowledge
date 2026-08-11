use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::project::root::{ProjectRoot, ProjectRootError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOption {
  pub label: String,
  pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItem {
  pub id: String,
  pub status: String,
  #[serde(rename = "type", default = "default_review_type")]
  pub review_type: String,
  pub title: String,
  pub description: String,
  #[serde(default)]
  pub source_path: Option<String>,
  #[serde(default)]
  pub affected_pages: Option<Vec<String>>,
  #[serde(default)]
  pub search_queries: Option<Vec<String>>,
  #[serde(default = "default_review_options")]
  pub options: Vec<ReviewOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReviewStore {
  pub reviews: Vec<ReviewItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSweepResult {
  pub resolved_ids: Vec<String>,
  pub unresolved_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBatchUpdateResult {
  pub updated_ids: Vec<String>,
  pub not_found_ids: Vec<String>,
  pub count: usize,
}

#[derive(Debug, Default)]
struct WikiIndex {
  by_id: BTreeSet<String>,
  by_title: BTreeSet<String>,
  pages: Vec<WikiPageSummary>,
}

#[derive(Debug, Clone)]
struct WikiPageSummary {
  id: String,
  title: Option<String>,
}

const REVIEW_OPENER_PREFIX: &str = "---REVIEW:";
const REVIEW_CLOSER: &str = "---END REVIEW---";

pub fn load_reviews(root: &ProjectRoot) -> Result<ReviewStore, ProjectRootError> {
  load_review_store_raw(root).map(normalize_review_store)
}

fn load_review_store_raw(root: &ProjectRoot) -> Result<ReviewStore, ProjectRootError> {
  let path = root.safe_join(".knowledge/reviews/items.json")?;
  let raw = fs::read_to_string(path)?;
  serde_json::from_str(&raw).map_err(|error| ProjectRootError::InvalidSourcePayload(error.to_string()))
}

pub fn save_reviews(root: &ProjectRoot, store: &ReviewStore) -> Result<(), ProjectRootError> {
  let path = root.safe_join(".knowledge/reviews/items.json")?;
  let normalized = normalize_review_store(store.clone());
  let json = serde_json::to_string(&normalized)
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

  let mut review = ReviewItem {
    id: String::new(),
    status: "open".to_string(),
    review_type: "suggestion".to_string(),
    title: format!("Review {source_name}"),
    description: "Potential unresolved knowledge gap detected during ingest.".to_string(),
    source_path: Some(format!("raw/sources/{source_name}")),
    affected_pages: None,
    search_queries: None,
    options: default_review_options(),
  };
  review.id = stable_review_id(&review);
  append_review_items(root, vec![review]).map(|_| ())
}

pub fn save_generated_reviews(
  root: &ProjectRoot,
  source_name: &str,
  generation_text: &str,
) -> Result<usize, ProjectRootError> {
  let parsed = parse_review_blocks(source_name, generation_text);
  if parsed.is_empty() {
    return Ok(0);
  }

  append_review_items(root, parsed).map(|ids| ids.len())
}

pub fn append_review_items(
  root: &ProjectRoot,
  reviews: Vec<ReviewItem>,
) -> Result<Vec<String>, ProjectRootError> {
  if reviews.is_empty() {
    return Ok(Vec::new());
  }

  let mut store = load_reviews(root)?;
  let mut ids = Vec::new();
  for review in reviews {
    let mut normalized = review;
    normalized.id = stable_review_id(&normalized);
    ids.push(normalized.id.clone());
    upsert_review_item(&mut store.reviews, normalized);
  }
  save_reviews(root, &store)?;
  Ok(ids)
}

pub fn update_review_status(
  root: &ProjectRoot,
  review_id: &str,
  status: &str,
) -> Result<ReviewItem, ProjectRootError> {
  let result = update_review_statuses(root, &[review_id.to_string()], status)?;
  let Some(updated_id) = result.updated_ids.first() else {
    return Err(ProjectRootError::InvalidSourcePayload("review not found".to_string()));
  };
  let store = load_reviews(root)?;
  store
    .reviews
    .into_iter()
    .find(|review| review.id == *updated_id)
    .ok_or_else(|| ProjectRootError::InvalidSourcePayload("review not found".to_string()))
}

pub fn update_review_statuses(
  root: &ProjectRoot,
  review_ids: &[String],
  status: &str,
) -> Result<ReviewBatchUpdateResult, ProjectRootError> {
  if status != "resolved" && status != "dismissed" && status != "open" {
    return Err(ProjectRootError::InvalidSourcePayload(format!(
      "unsupported review status {status}"
    )));
  }

  let raw_store = load_review_store_raw(root)?;
  let mut id_aliases = BTreeMap::new();
  for review in &raw_store.reviews {
    id_aliases.insert(review.id.clone(), stable_review_id(review));
  }
  let mut store = normalize_review_store(raw_store);
  let mut updated_ids = Vec::new();
  let mut not_found_ids = Vec::new();
  let mut changed = false;

  for requested_id in review_ids {
    let stable_requested_id = id_aliases.get(requested_id).unwrap_or(requested_id);
    let Some(review) = store
      .reviews
      .iter_mut()
      .find(|review| review.id == *stable_requested_id || stable_review_id(review) == *stable_requested_id)
    else {
      not_found_ids.push(requested_id.clone());
      continue;
    };

    let stable_id = stable_review_id(review);
    review.id = stable_id.clone();
    if review.status != status {
      review.status = status.to_string();
    }
    updated_ids.push(stable_id);
    changed = true;
  }

  if changed {
    store = normalize_review_store(store);
    save_reviews(root, &store)?;
  }

  Ok(ReviewBatchUpdateResult {
    count: updated_ids.len(),
    updated_ids,
    not_found_ids,
  })
}

pub fn sweep_resolved_reviews(root: &ProjectRoot) -> Result<ReviewSweepResult, ProjectRootError> {
  let mut store = load_reviews(root)?;
  let index = build_wiki_index(root)?;
  let mut resolved_ids = Vec::new();
  let mut unresolved_ids = Vec::new();

  for review in &mut store.reviews {
    if review.status != "open" {
      continue;
    }

    if should_auto_resolve_review(review, &index) {
      review.status = "resolved".to_string();
      resolved_ids.push(review.id.clone());
    } else {
      unresolved_ids.push(review.id.clone());
    }
  }

  if !resolved_ids.is_empty() {
    save_reviews(root, &store)?;
  }

  Ok(ReviewSweepResult {
    resolved_ids,
    unresolved_ids,
  })
}

pub fn build_review_sweep_prompt(
  root: &ProjectRoot,
  review_ids: &[String],
  max_pages: usize,
) -> Result<Option<String>, ProjectRootError> {
  let store = load_reviews(root)?;
  let pending = store
    .reviews
    .into_iter()
    .filter(|review| review.status == "open" && review_ids.iter().any(|id| id == &review.id))
    .collect::<Vec<_>>();
  if pending.is_empty() {
    return Ok(None);
  }

  let index = build_wiki_index(root)?;
  let page_list = index
    .pages
    .iter()
    .take(max_pages)
    .map(|page| match &page.title {
      Some(title) => format!("- {} (title: {})", page.id, title),
      None => format!("- {}", page.id),
    })
    .collect::<Vec<_>>()
    .join("\n");
  let review_list = pending
    .iter()
    .map(|review| {
      let affected = review
        .affected_pages
        .as_ref()
        .filter(|pages| !pages.is_empty())
        .map(|pages| format!(" | affected: {}", pages.join(", ")))
        .unwrap_or_default();
      let description = if review.description.trim().is_empty() {
        String::new()
      } else {
        format!(" | description: {}", review.description.trim())
      };
      format!(
        "- id={} [{}] \"{}\"{}{}",
        review.id, review.review_type, review.title, description, affected
      )
    })
    .collect::<Vec<_>>()
    .join("\n");

  Ok(Some(
    [
      "You are cleaning up stale review items for a personal wiki.",
      "Decide whether each review item has already been resolved by the current wiki state.",
      "Be conservative. Only resolve an item if the concern no longer applies.",
      "Contradiction, confirm, and human-judgment items should usually stay unresolved.",
      "",
      "Current wiki pages:",
      if page_list.is_empty() { "(no pages yet)" } else { &page_list },
      "",
      "Pending review items:",
      &review_list,
      "",
      "Respond with JSON only in this exact shape:",
      "{\"resolved\": [\"review-id-1\", \"review-id-2\"]}",
      "Return an empty array when nothing should be resolved.",
    ]
    .join("\n"),
  ))
}

pub fn parse_review_resolution_ids(raw: &str, valid_review_ids: &[String]) -> Vec<String> {
  let Some(json_object) = extract_json_object(raw) else {
    return Vec::new();
  };
  let Ok(value) = serde_json::from_str::<serde_json::Value>(&json_object) else {
    return Vec::new();
  };
  let Some(ids) = value.get("resolved").and_then(serde_json::Value::as_array) else {
    return Vec::new();
  };
  let valid = valid_review_ids.iter().collect::<BTreeSet<_>>();
  ids
    .iter()
    .filter_map(serde_json::Value::as_str)
    .filter(|id| valid.contains(&id.to_string()))
    .map(str::to_string)
    .collect::<Vec<_>>()
}

pub fn resolve_review_ids(
  root: &ProjectRoot,
  review_ids: &[String],
) -> Result<ReviewSweepResult, ProjectRootError> {
  let result = update_review_statuses(root, review_ids, "resolved")?;
  Ok(ReviewSweepResult {
    resolved_ids: result.updated_ids,
    unresolved_ids: result.not_found_ids,
  })
}

pub fn _review_path(root: &Path) -> std::path::PathBuf {
  root.join(".knowledge/reviews/items.json")
}

fn parse_review_blocks(source_name: &str, text: &str) -> Vec<ReviewItem> {
  let normalized = text.replace("\r\n", "\n");
  let mut reviews = Vec::new();
  let mut remaining = normalized.as_str();

  while let Some(start) = remaining.find(REVIEW_OPENER_PREFIX) {
    remaining = &remaining[start + REVIEW_OPENER_PREFIX.len()..];
    let Some(header_end) = remaining.find("---\n") else {
      break;
    };
    let header = remaining[..header_end].trim();
    remaining = &remaining[header_end + 4..];

    let Some(block_end) = remaining.find(REVIEW_CLOSER) else {
      break;
    };
    let body = remaining[..block_end].trim();
    remaining = &remaining[block_end + REVIEW_CLOSER.len()..];

    let Some((raw_type, raw_title)) = header.split_once('|') else {
      continue;
    };
    let review_type = raw_type.trim().to_lowercase();
    let title = raw_title.trim();
    if title.is_empty() {
      continue;
    }

    let options = parse_review_options(body);
    let affected_pages = parse_review_pages(body);
    let search_queries = parse_review_queries(body);
    let description = body
      .lines()
      .filter(|line| {
        let trimmed = line.trim_start();
        !trimmed.starts_with("OPTIONS:")
          && !trimmed.starts_with("PAGES:")
          && !trimmed.starts_with("SEARCH:")
      })
      .collect::<Vec<_>>()
      .join("\n")
      .trim()
      .to_string();

    let mut review = ReviewItem {
      id: String::new(),
      status: "open".to_string(),
      review_type: normalize_review_type(&review_type).to_string(),
      title: title.to_string(),
      description: if description.is_empty() {
        title.to_string()
      } else {
        description
      },
      source_path: Some(format!("raw/sources/{source_name}")),
      affected_pages,
      search_queries,
      options,
    };
    review.id = stable_review_id(&review);
    reviews.push(review);
  }

  reviews
}

fn build_wiki_index(root: &ProjectRoot) -> Result<WikiIndex, ProjectRootError> {
  let wiki_root = root.safe_join("wiki")?;
  let mut index = WikiIndex::default();
  collect_wiki_index(&wiki_root, &mut index)?;
  Ok(index)
}

fn collect_wiki_index(root: &Path, index: &mut WikiIndex) -> Result<(), ProjectRootError> {
  if !root.exists() {
    return Ok(());
  }

  for entry in fs::read_dir(root)? {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      collect_wiki_index(&path, index)?;
      continue;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
      continue;
    }

    let file_stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();
    if !file_stem.is_empty() {
      index.by_id.insert(file_stem.to_lowercase());
    }

    let content = fs::read_to_string(&path)?;
    let title = extract_frontmatter_field(&content, "title");
    if let Some(title) = &title {
      index.by_title.insert(title.to_lowercase());
    }
    index.pages.push(WikiPageSummary {
      id: file_stem.to_lowercase(),
      title,
    });
  }

  Ok(())
}

pub fn stable_review_id(review: &ReviewItem) -> String {
  review_id_for_parts(&review.review_type, &review.title)
}

pub fn review_id_for_parts(review_type: &str, title: &str) -> String {
  // upstream_llm_wiki/src/stores/review-store.ts reviewIdFor (L49-57):
  // FNV-1a over UTF-16 code units to match JavaScript charCodeAt.
  let key = format!("{}::{}", review_type, normalize_review_title(title));
  let mut hash = 0x811c9dc5u32;
  for unit in key.encode_utf16() {
    hash ^= u32::from(unit);
    hash = hash.wrapping_mul(0x0100_0193);
  }
  format!("review-{hash:08x}")
}

fn normalize_review_store(store: ReviewStore) -> ReviewStore {
  let mut reviews = Vec::new();
  for mut review in store.reviews {
    review.id = stable_review_id(&review);
    upsert_review_item(&mut reviews, review);
  }
  ReviewStore { reviews }
}

fn upsert_review_item(reviews: &mut Vec<ReviewItem>, review: ReviewItem) {
  if let Some(existing) = reviews.iter_mut().find(|existing| existing.id == review.id) {
    *existing = merge_review_items(existing.clone(), review);
  } else {
    reviews.push(review);
  }
}

fn merge_review_items(a: ReviewItem, b: ReviewItem) -> ReviewItem {
  let status = merge_review_status(&a.status, &b.status).to_string();
  ReviewItem {
    id: a.id,
    status,
    review_type: a.review_type,
    title: a.title,
    description: if a.description.trim().is_empty() {
      b.description
    } else {
      a.description
    },
    source_path: a.source_path.or(b.source_path),
    affected_pages: union_optional_strings(a.affected_pages, b.affected_pages),
    search_queries: union_optional_strings(a.search_queries, b.search_queries),
    options: merge_review_options(a.options, b.options),
  }
}

fn merge_review_status(a: &str, b: &str) -> &'static str {
  if a == "resolved" || b == "resolved" {
    "resolved"
  } else if a == "dismissed" || b == "dismissed" {
    "dismissed"
  } else {
    "open"
  }
}

fn union_optional_strings(a: Option<Vec<String>>, b: Option<Vec<String>>) -> Option<Vec<String>> {
  let mut values = Vec::new();
  for value in a.into_iter().flatten().chain(b.into_iter().flatten()) {
    if !values.iter().any(|existing| existing == &value) {
      values.push(value);
    }
  }
  if values.is_empty() { None } else { Some(values) }
}

fn merge_review_options(a: Vec<ReviewOption>, b: Vec<ReviewOption>) -> Vec<ReviewOption> {
  let mut by_action = BTreeMap::new();
  for option in a.into_iter().chain(b) {
    by_action.entry(option.action.clone()).or_insert(option);
  }
  by_action.into_values().collect()
}

fn should_auto_resolve_review(review: &ReviewItem, index: &WikiIndex) -> bool {
  match review.review_type.as_str() {
    "missing-page" => extract_candidate_page_names(review)
      .iter()
      .any(|candidate| page_exists(candidate, index)),
    "duplicate" => review
      .affected_pages
      .as_ref()
      .filter(|pages| !pages.is_empty())
      .map(|pages| !pages.iter().all(|page| page_path_exists(page, index)))
      .unwrap_or(false),
    _ => false,
  }
}

fn extract_candidate_page_names(review: &ReviewItem) -> Vec<String> {
  let normalized = normalize_review_title(&review.title);
  if normalized.is_empty() {
    Vec::new()
  } else {
    vec![normalized]
  }
}

fn normalize_review_title(title: &str) -> String {
  // upstream_llm_wiki/src/lib/review-utils.ts normalizeReviewTitle (L21-28).
  let trimmed = title.trim_start();
  if trimmed.is_empty() {
    return String::new();
  }

  let prefixes = [
    "missing page",
    "missing-page",
    "missingpage",
    "duplicate page",
    "duplicate-page",
    "duplicatepage",
    "possible duplicate",
    "possible-duplicate",
    "possibleduplicate",
    "缺失页面",
    "缺少页面",
    "重复页面",
    "疑似重复",
  ];
  let lower = trimmed.to_lowercase();
  for prefix in prefixes {
    if let Some(rest) = lower.strip_prefix(prefix) {
      let original_rest = &trimmed[trimmed.len() - rest.len()..];
      if original_rest.starts_with(':') || original_rest.starts_with('：') {
        let stripped = original_rest
          .trim_start_matches(':')
          .trim_start_matches('：')
          .trim();
        return collapse_whitespace(&stripped.to_lowercase());
      }
    }
  }

  collapse_whitespace(&lower)
}

fn collapse_whitespace(value: &str) -> String {
  value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn page_exists(candidate: &str, index: &WikiIndex) -> bool {
  let normalized = candidate.trim().to_lowercase();
  if normalized.is_empty() {
    return false;
  }

  index.by_id.contains(&normalized)
    || index.by_id.contains(&normalized.replace(' ', "-"))
    || index.by_title.contains(&normalized)
}

fn page_path_exists(path: &str, index: &WikiIndex) -> bool {
  let stem = Path::new(path)
    .file_stem()
    .and_then(|value| value.to_str())
    .unwrap_or_default()
    .to_lowercase();
  if stem.is_empty() {
    return false;
  }
  index.by_id.contains(&stem)
}

fn extract_frontmatter_field(content: &str, field: &str) -> Option<String> {
  let mut lines = content.lines();
  if lines.next()?.trim() != "---" {
    return None;
  }

  for line in lines {
    let trimmed = line.trim();
    if trimmed == "---" {
      break;
    }
    let Some((key, value)) = trimmed.split_once(':') else {
      continue;
    };
    if key.trim().eq_ignore_ascii_case(field) {
      let normalized = value.trim().trim_matches('"').trim_matches('\'').trim();
      if normalized.is_empty() {
        return None;
      }
      return Some(normalized.to_string());
    }
  }

  None
}

fn extract_json_object(raw: &str) -> Option<String> {
  let trimmed = raw
    .trim()
    .trim_start_matches("```json")
    .trim_start_matches("```")
    .trim_end_matches("```")
    .trim();
  let start = trimmed.find('{')?;
  let mut depth = 0usize;
  let mut in_string = false;
  let mut escaped = false;

  for (offset, ch) in trimmed[start..].char_indices() {
    if escaped {
      escaped = false;
      continue;
    }
    if in_string && ch == '\\' {
      escaped = true;
      continue;
    }
    if ch == '"' {
      in_string = !in_string;
      continue;
    }
    if in_string {
      continue;
    }
    if ch == '{' {
      depth += 1;
      continue;
    }
    if ch == '}' {
      depth = depth.saturating_sub(1);
      if depth == 0 {
        return Some(trimmed[start..start + offset + ch.len_utf8()].to_string());
      }
    }
  }

  None
}

fn normalize_review_type(value: &str) -> &'static str {
  match value {
    "contradiction" => "contradiction",
    "duplicate" => "duplicate",
    "missing-page" => "missing-page",
    "suggestion" => "suggestion",
    _ => "confirm",
  }
}

fn default_review_type() -> String {
  "suggestion".to_string()
}

fn parse_review_options(body: &str) -> Vec<ReviewOption> {
  let Some(line) = body.lines().find(|line| line.trim_start().starts_with("OPTIONS:")) else {
    return default_review_options();
  };

  let options = line
    .trim_start()
    .trim_start_matches("OPTIONS:")
    .split('|')
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(|value| ReviewOption {
      label: value.to_string(),
      action: value.to_string(),
    })
    .collect::<Vec<_>>();

  if options.is_empty() {
    default_review_options()
  } else {
    options
  }
}

fn parse_review_pages(body: &str) -> Option<Vec<String>> {
  let line = body.lines().find(|line| line.trim_start().starts_with("PAGES:"))?;
  let pages = line
    .trim_start()
    .trim_start_matches("PAGES:")
    .split(',')
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_string)
    .collect::<Vec<_>>();

  if pages.is_empty() {
    None
  } else {
    Some(pages)
  }
}

fn parse_review_queries(body: &str) -> Option<Vec<String>> {
  let line = body.lines().find(|line| line.trim_start().starts_with("SEARCH:"))?;
  let queries = line
    .trim_start()
    .trim_start_matches("SEARCH:")
    .split('|')
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_string)
    .collect::<Vec<_>>();

  if queries.is_empty() {
    None
  } else {
    Some(queries)
  }
}

fn default_review_options() -> Vec<ReviewOption> {
  vec![
    ReviewOption {
      label: "Approve".to_string(),
      action: "Approve".to_string(),
    },
    ReviewOption {
      label: "Skip".to_string(),
      action: "Skip".to_string(),
    },
  ]
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use crate::project::scaffold::initialize_project;

  use super::{
    ReviewItem, ReviewOption, ReviewStore, default_review_options, maybe_add_review_for_source,
    normalize_review_title, parse_review_blocks, review_id_for_parts, save_reviews, stable_review_id,
    sweep_resolved_reviews, update_review_statuses,
  };

  #[test]
  fn review_store_deserializes_legacy_items_without_structured_fields() {
    let store: ReviewStore = serde_json::from_str(
      r#"{
        "reviews": [
          {
            "id": "review-legacy",
            "status": "open",
            "title": "Legacy review",
            "description": "Created before structured review metadata existed."
          }
        ]
      }"#,
    )
    .expect("legacy reviews should deserialize");

    assert_eq!(store.reviews.len(), 1);
    let review = &store.reviews[0];
    assert_eq!(review.review_type, "suggestion");
    assert_eq!(review.source_path, None);
    assert_eq!(review.affected_pages, None);
    assert_eq!(review.search_queries, None);
    assert_eq!(review.options.len(), default_review_options().len());
  }

  #[test]
  fn stable_review_ids_normalize_common_title_prefixes() {
    let english = review_id_for_parts("missing-page", "Missing page: Attention Mechanism");
    let chinese = review_id_for_parts("missing-page", "缺失页面： Attention Mechanism");
    let bare = review_id_for_parts("missing-page", "Attention   Mechanism");
    let duplicate = review_id_for_parts("duplicate", "Attention Mechanism");

    assert_eq!(english, chinese);
    assert_eq!(english, bare);
    assert_ne!(english, duplicate);
    assert_eq!(normalize_review_title("Missing page Attention"), "missing page attention");
    assert_eq!(normalize_review_title("疑似重复 注意力"), "疑似重复 注意力");
  }

  #[test]
  fn load_reviews_migrates_legacy_ids_and_merges_duplicates() {
    let temp = tempdir().expect("tempdir");
    let root = initialize_project(temp.path()).expect("project");
    save_reviews(
      &root,
      &ReviewStore {
        reviews: vec![
          ReviewItem {
            id: "review-old-open".to_string(),
            status: "open".to_string(),
            review_type: "missing-page".to_string(),
            title: "Attention".to_string(),
            description: "".to_string(),
            source_path: Some("raw/sources/a.md".to_string()),
            affected_pages: Some(vec!["wiki/a.md".to_string()]),
            search_queries: Some(vec!["attention".to_string()]),
            options: vec![ReviewOption {
              label: "Open".to_string(),
              action: "open:a".to_string(),
            }],
          },
          ReviewItem {
            id: "review-old-resolved".to_string(),
            status: "resolved".to_string(),
            review_type: "missing-page".to_string(),
            title: "Missing page: Attention".to_string(),
            description: "Resolved copy".to_string(),
            source_path: None,
            affected_pages: Some(vec!["wiki/b.md".to_string()]),
            search_queries: Some(vec!["attention".to_string(), "transformer".to_string()]),
            options: vec![ReviewOption {
              label: "Skip".to_string(),
              action: "Skip".to_string(),
            }],
          },
        ],
      },
    )
    .expect("save reviews");

    let store = super::load_reviews(&root).expect("load reviews");
    assert_eq!(store.reviews.len(), 1);
    let review = &store.reviews[0];
    assert_eq!(review.id, review_id_for_parts("missing-page", "Attention"));
    assert_eq!(review.status, "resolved");
    assert_eq!(review.affected_pages.as_ref().expect("pages").len(), 2);
    assert_eq!(review.search_queries.as_ref().expect("queries").len(), 2);
    assert_eq!(review.options.len(), 2);
  }

  #[test]
  fn batch_update_accepts_legacy_ids_and_reports_not_found() {
    let temp = tempdir().expect("tempdir");
    let root = initialize_project(temp.path()).expect("project");
    let review = ReviewItem {
      id: "review-legacy-id".to_string(),
      status: "open".to_string(),
      review_type: "suggestion".to_string(),
      title: "Review legacy source".to_string(),
      description: "Needs attention".to_string(),
      source_path: None,
      affected_pages: None,
      search_queries: None,
      options: default_review_options(),
    };
    let stable_id = stable_review_id(&review);
    std::fs::write(
      temp.path().join(".knowledge/reviews/items.json"),
      serde_json::to_string(&ReviewStore { reviews: vec![review] }).expect("json"),
    )
    .expect("legacy review sidecar");

    let result = update_review_statuses(
      &root,
      &["review-legacy-id".to_string(), "review-missing".to_string()],
      "dismissed",
    )
    .expect("batch update");

    assert_eq!(result.updated_ids, vec![stable_id.clone()]);
    assert_eq!(result.not_found_ids, vec!["review-missing".to_string()]);
    assert_eq!(result.count, 1);

    let store = super::load_reviews(&root).expect("load reviews");
    assert_eq!(store.reviews[0].id, stable_id);
    assert_eq!(store.reviews[0].status, "dismissed");
  }

  #[test]
  fn generated_reviews_use_stable_ids() {
    let parsed = parse_review_blocks(
      "source.md",
      "---REVIEW:missing-page|Missing page: Attention---\nCreate it\n---END REVIEW---",
    );
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].id, review_id_for_parts("missing-page", "Attention"));

    let temp = tempdir().expect("tempdir");
    let root = initialize_project(temp.path()).expect("project");
    maybe_add_review_for_source(&root, "source.md", "missing question")
      .expect("add review");
    let store = super::load_reviews(&root).expect("load reviews");
    assert_eq!(store.reviews[0].id, stable_review_id(&store.reviews[0]));
  }

  #[test]
  fn review_sweep_resolves_missing_page_and_duplicate_rules() {
    let temp = tempdir().expect("tempdir");
    let root = initialize_project(temp.path()).expect("project");

    std::fs::write(
      temp.path().join("wiki/concepts/attention-mechanism.md"),
      [
        "---",
        "type: concept",
        "title: Attention Mechanism",
        "sources: []",
        "---",
        "",
        "# Attention Mechanism",
      ]
      .join("\n"),
    )
    .expect("concept page");
    std::fs::write(
      temp.path().join("wiki/concepts/attention.md"),
      [
        "---",
        "type: concept",
        "title: Attention",
        "sources: []",
        "---",
        "",
        "# Attention",
      ]
      .join("\n"),
    )
    .expect("attention page");

    save_reviews(
      &root,
      &ReviewStore {
        reviews: vec![
          ReviewItem {
            id: "review-missing".to_string(),
            status: "open".to_string(),
            review_type: "missing-page".to_string(),
            title: "Missing page: attention mechanism".to_string(),
            description: "The concept page is missing.".to_string(),
            source_path: None,
            affected_pages: None,
            search_queries: None,
            options: default_review_options(),
          },
          ReviewItem {
            id: "review-duplicate".to_string(),
            status: "open".to_string(),
            review_type: "duplicate".to_string(),
            title: "Duplicate page: attention concept split".to_string(),
            description: "The duplicate page was deleted already.".to_string(),
            source_path: None,
            affected_pages: Some(vec![
              "wiki/concepts/attention.md".to_string(),
              "wiki/concepts/attention-v2.md".to_string(),
            ]),
            search_queries: None,
            options: default_review_options(),
          },
        ],
      },
    )
    .expect("save reviews");

    let result = sweep_resolved_reviews(&root).expect("sweep reviews");
    assert_eq!(result.resolved_ids.len(), 2);

    let store = super::load_reviews(&root).expect("load reviews");
    assert_eq!(store.reviews[0].status, "resolved");
    assert_eq!(store.reviews[1].status, "resolved");
  }
}
