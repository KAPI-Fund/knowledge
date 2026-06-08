use std::collections::BTreeSet;
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
    review_type: "suggestion".to_string(),
    title: format!("Review {source_name}"),
    description: "Potential unresolved knowledge gap detected during ingest.".to_string(),
    source_path: Some(format!("raw/sources/{source_name}")),
    affected_pages: None,
    search_queries: None,
    options: default_review_options(),
  });
  save_reviews(root, &store)
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

  let mut store = load_reviews(root)?;
  let mut inserted = 0usize;

  for review in parsed {
    if store.reviews.iter().any(|item| item.id == review.id) {
      continue;
    }
    store.reviews.push(review);
    inserted += 1;
  }

  if inserted > 0 {
    save_reviews(root, &store)?;
  }

  Ok(inserted)
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
  let mut store = load_reviews(root)?;
  let wanted = review_ids.iter().cloned().collect::<BTreeSet<_>>();
  let mut resolved_ids = Vec::new();
  let mut unresolved_ids = Vec::new();

  for review in &mut store.reviews {
    if review.status != "open" {
      continue;
    }
    if wanted.contains(&review.id) {
      review.status = "resolved".to_string();
      resolved_ids.push(review.id.clone());
    }
  }

  for review_id in review_ids {
    if !resolved_ids.iter().any(|resolved| resolved == review_id) {
      unresolved_ids.push(review_id.clone());
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

pub fn _review_path(root: &Path) -> std::path::PathBuf {
  root.join(".knowledge/reviews/items.json")
}

fn parse_review_blocks(source_name: &str, text: &str) -> Vec<ReviewItem> {
  let normalized = text.replace("\r\n", "\n");
  let mut reviews = Vec::new();
  let mut remaining = normalized.as_str();
  let source_stem = source_name.trim_end_matches(".md");

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

    reviews.push(ReviewItem {
      id: format!(
        "review-{}-{}-{}",
        source_stem,
        slugify(&review_type),
        slugify(title)
      ),
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
    });
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
  let trimmed = title.trim();
  if trimmed.is_empty() {
    return String::new();
  }

  let lower = trimmed.to_lowercase();
  let prefixes = [
    "missing page:",
    "missing-page:",
    "duplicate page:",
    "possible duplicate:",
  ];

  for prefix in prefixes {
    if let Some(stripped) = lower.strip_prefix(prefix) {
      return collapse_whitespace(stripped);
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

fn slugify(value: &str) -> String {
  let mut slug = String::new();
  let mut last_dash = false;

  for ch in value.chars() {
    if ch.is_ascii_alphanumeric() {
      slug.push(ch.to_ascii_lowercase());
      last_dash = false;
    } else if !last_dash && !slug.is_empty() {
      slug.push('-');
      last_dash = true;
    }
  }

  slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use crate::project::scaffold::initialize_project;

  use super::{ReviewItem, ReviewStore, default_review_options, save_reviews, sweep_resolved_reviews};

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
