use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use knowledge_core::project::root::ProjectRoot;
use knowledge_core::search::{
  extract_image_refs, extract_title, search_project_with_options, ProjectSearchResponse,
  SearchOptions, SearchResult,
};
use sha2::{Digest, Sha256};

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::providers::{OpenAiCompatibleProvider, ProviderEmbeddingRequest, ProviderError};
use crate::retrieval::chunker::{chunk_markdown, ChunkingOptions};
use crate::retrieval::store::{
  delete_missing_pages, load_page_hashes, load_project_chunks, replace_page_chunks, PageIndexState,
  StoredChunk, StoredChunkInput,
};

const STRUCTURAL_PAGE_IDS: &[&str] = &["index", "log", "overview", "schema", "purpose"];
const RRF_K: f64 = 60.0;

#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
  pub base_url: String,
  pub api_key: String,
  pub model: String,
  pub timeout_seconds: i64,
}

#[derive(Debug, Clone)]
struct PageVectorResult {
  id: String,
  relative_path: String,
  title: String,
  score: f32,
  chunk_text: String,
  heading_path: String,
}

/// Pure: build an embedding config from the raw embedding_* columns. Returns
/// None when disabled or incomplete (matching the loader's Option contract).
pub fn embedding_config_from_row(
  enabled: bool,
  base_url: Option<String>,
  api_key: Option<String>,
  model: Option<String>,
  timeout_seconds: Option<i64>,
) -> Option<EmbeddingConfig> {
  if !enabled {
    return None;
  }
  let base_url = base_url.unwrap_or_default();
  let model = model.unwrap_or_default();
  if base_url.trim().is_empty() || model.trim().is_empty() {
    return None;
  }
  Some(EmbeddingConfig {
    base_url,
    api_key: api_key.unwrap_or_default(),
    model,
    timeout_seconds: timeout_seconds.unwrap_or(30),
  })
}

pub async fn load_embedding_config(state: &AppState) -> Result<Option<EmbeddingConfig>, ApiError> {
  let (enabled, base_url, api_key, model, timeout_seconds) =
    sqlx::query_as::<_, (bool, Option<String>, Option<String>, Option<String>, Option<i64>)>(
      "SELECT
         embedding_enabled,
         embedding_base_url,
         embedding_api_key,
         embedding_model,
         embedding_timeout_seconds
       FROM system_settings
       WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

  Ok(embedding_config_from_row(enabled, base_url, api_key, model, timeout_seconds))
}

pub async fn search_project_hybrid(
  state: &AppState,
  project_id: &str,
  root: &ProjectRoot,
  query: &str,
  options: SearchOptions,
) -> Result<ProjectSearchResponse, ApiError> {
  let keyword = search_project_with_options(root.as_path(), query, options)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  let Some(config) = load_embedding_config(state).await? else {
    return Ok(keyword);
  };

  if let Err(error) = ensure_project_embeddings(state, project_id, root, &config).await {
    tracing::warn!("embedding index refresh failed: {error}");
    return Ok(keyword);
  }

  let provider = OpenAiCompatibleProvider::new(
    config.base_url.clone(),
    config.api_key.clone(),
    config.model.clone(),
    config.timeout_seconds,
  );
  let query_embedding = match provider
    .embed_text(ProviderEmbeddingRequest {
      text: query.to_string(),
    })
    .await
  {
    Ok(embedding) => embedding,
    Err(error) => {
      tracing::warn!("query embedding failed: {}", error.message());
      return Ok(keyword);
    }
  };

  let stored_chunks = load_project_chunks(&state.pool, project_id).await?;
  let vector_results = rank_vector_results(&stored_chunks, &query_embedding, options.top_k.max(10));
  if vector_results.is_empty() {
    return Ok(keyword);
  }

  Ok(merge_keyword_and_vector_results(
    root.as_path(),
    keyword,
    vector_results,
    options.include_content,
  ))
}

pub(crate) async fn ensure_project_embeddings(
  state: &AppState,
  project_id: &str,
  root: &ProjectRoot,
  config: &EmbeddingConfig,
) -> Result<(), ApiError> {
  let existing = load_page_hashes(&state.pool, project_id).await?;
  let pages = load_wiki_pages(root.as_path())?;
  let keep_page_ids = pages.iter().map(|page| page.page_id.clone()).collect::<BTreeSet<_>>();
  let provider = OpenAiCompatibleProvider::new(
    config.base_url.clone(),
    config.api_key.clone(),
    config.model.clone(),
    config.timeout_seconds,
  );

  for page in pages {
    if page_is_current(existing.get(&page.page_id), &page.content_hash, &config.model) {
      continue;
    }

    let chunks = build_page_chunks(&provider, &page, &config.model).await?;
    if chunks.is_empty() {
      continue;
    }

    replace_page_chunks(&state.pool, project_id, &page.page_id, &chunks).await?;
  }

  delete_missing_pages(&state.pool, project_id, &keep_page_ids).await?;
  Ok(())
}

fn page_is_current(
  existing: Option<&PageIndexState>,
  content_hash: &str,
  embedding_model: &str,
) -> bool {
  existing
    .map(|state| state.content_hash == content_hash && state.embedding_model == embedding_model)
    .unwrap_or(false)
}

async fn build_page_chunks(
  provider: &OpenAiCompatibleProvider,
  page: &PageDocument,
  embedding_model: &str,
) -> Result<Vec<StoredChunkInput>, ApiError> {
  let chunks = chunk_markdown(&page.content, ChunkingOptions::default());
  let mut rows = Vec::new();

  for chunk in chunks {
    let text = enrich_chunk_for_embedding(&page.title, &chunk.heading_path, &chunk.text);
    let embedding = provider
      .embed_text(ProviderEmbeddingRequest { text })
      .await
      .map_err(map_provider_error)?;

    rows.push(StoredChunkInput {
      page_id: page.page_id.clone(),
      relative_path: page.relative_path.clone(),
      title: page.title.clone(),
      embedding_model: embedding_model.to_string(),
      chunk_index: chunk.index,
      heading_path: chunk.heading_path,
      chunk_text: chunk.text,
      content_hash: page.content_hash.clone(),
      embedding,
    });
  }

  Ok(rows)
}

fn enrich_chunk_for_embedding(title: &str, heading_path: &str, chunk_text: &str) -> String {
  let mut parts = Vec::new();
  if !title.trim().is_empty() {
    parts.push(title.trim().to_string());
  }
  if !heading_path.trim().is_empty() {
    parts.push(heading_path.trim().to_string());
  }
  parts.push(chunk_text.trim().to_string());
  parts.join("\n\n")
}

fn rank_vector_results(
  stored_chunks: &[StoredChunk],
  query_embedding: &[f32],
  top_k: usize,
) -> Vec<PageVectorResult> {
  let mut ranked_chunks = stored_chunks
    .iter()
    .filter_map(|chunk| {
      cosine_similarity(query_embedding, &chunk.embedding).map(|score| (chunk, score))
    })
    .collect::<Vec<_>>();

  ranked_chunks.sort_by(|left, right| {
    right
      .1
      .partial_cmp(&left.1)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| left.0.relative_path.cmp(&right.0.relative_path))
      .then_with(|| left.0.chunk_index.cmp(&right.0.chunk_index))
  });
  ranked_chunks.truncate((top_k * 3).max(30));

  let mut by_page: BTreeMap<String, Vec<(&StoredChunk, f32)>> = BTreeMap::new();
  for (chunk, score) in ranked_chunks {
    by_page.entry(chunk.page_id.clone()).or_default().push((chunk, score));
  }

  let mut pages = Vec::new();
  for (page_id, mut chunks) in by_page {
    chunks.sort_by(|left, right| {
      right
        .1
        .partial_cmp(&left.1)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| left.0.chunk_index.cmp(&right.0.chunk_index))
    });
    let Some((top_chunk, top_score)) = chunks.first().copied() else {
      continue;
    };
    let tail_sum = chunks.iter().skip(1).map(|(_, score)| *score).sum::<f32>();
    let blended = top_score + (tail_sum * 0.3).min((1.0 - top_score).max(0.0));

    pages.push(PageVectorResult {
      id: page_id,
      relative_path: top_chunk.relative_path.clone(),
      title: top_chunk.title.clone(),
      score: blended,
      chunk_text: top_chunk.chunk_text.clone(),
      heading_path: top_chunk.heading_path.clone(),
    });
  }

  pages.sort_by(|left, right| {
    right
      .score
      .partial_cmp(&left.score)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| left.relative_path.cmp(&right.relative_path))
  });
  pages.truncate(top_k);
  pages
}

fn merge_keyword_and_vector_results(
  project_root: &Path,
  keyword: ProjectSearchResponse,
  vector_results: Vec<PageVectorResult>,
  include_content: bool,
) -> ProjectSearchResponse {
  let mut results_by_path = keyword
    .results
    .into_iter()
    .map(|result| (normalize_path(&result.path), result))
    .collect::<BTreeMap<_, _>>();
  let mut token_rank = BTreeMap::new();
  for (index, path) in results_by_path.keys().enumerate() {
    token_rank.insert(path.clone(), index + 1);
  }

  let mut vector_rank = BTreeMap::new();
  let mut vector_scores = BTreeMap::new();
  let mut vector_hits = 0usize;

  for (index, result) in vector_results.iter().enumerate() {
    vector_rank.insert(result.id.clone(), index + 1);
    vector_scores.insert(result.id.clone(), result.score);
    vector_hits += 1;

    let key = normalize_path(&result.relative_path);
    results_by_path.entry(key).or_insert_with(|| {
      materialize_vector_only_result(project_root, result, include_content)
    });
  }

  let mut merged = results_by_path.into_values().collect::<Vec<_>>();
  if vector_hits == 0 {
    return ProjectSearchResponse {
      mode: "keyword".to_string(),
      token_hits: keyword.token_hits,
      vector_hits,
      results: merged,
    };
  }

  apply_rrf_scores(&mut merged, &token_rank, &vector_rank, &vector_scores);
  merged.sort_by(|left, right| {
    right
      .score
      .partial_cmp(&left.score)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| left.path.cmp(&right.path))
  });

  ProjectSearchResponse {
    mode: search_mode(keyword.token_hits == 0, vector_hits).to_string(),
    token_hits: keyword.token_hits,
    vector_hits,
    results: merged,
  }
}

fn materialize_vector_only_result(
  project_root: &Path,
  result: &PageVectorResult,
  include_content: bool,
) -> SearchResult {
  let absolute = project_root.join(&result.relative_path);
  let content = fs::read_to_string(&absolute).ok();
  let images = content
    .as_deref()
    .map(extract_image_refs)
    .unwrap_or_default();

  SearchResult {
    path: result.relative_path.clone(),
    title: result.title.clone(),
    snippet: build_vector_snippet(&result.chunk_text, &result.heading_path),
    title_match: false,
    score: 0.0,
    vector_score: Some(result.score),
    images,
    content: include_content.then_some(content.unwrap_or_default()),
  }
}

fn build_vector_snippet(chunk_text: &str, heading_path: &str) -> String {
  let mut text = chunk_text.trim().replace('\n', " ");
  if text.is_empty() {
    return String::new();
  }
  if text.chars().count() > 160 {
    text = text.chars().take(160).collect::<String>();
    text.push_str("...");
  }

  let heading = heading_path.trim();
  if heading.is_empty() {
    text
  } else {
    format!("{heading}: {text}")
  }
}

fn apply_rrf_scores(
  results: &mut [SearchResult],
  token_rank: &BTreeMap<String, usize>,
  vector_rank: &BTreeMap<String, usize>,
  vector_scores: &BTreeMap<String, f32>,
) {
  for result in results {
    let key = normalize_path(&result.path);
    let token = token_rank.get(&key).copied();
    let vector = vector_rank.get(&key).copied();
    let mut rrf = 0.0;
    if let Some(rank) = token {
      rrf += 1.0 / (RRF_K + rank as f64);
    }
    if let Some(rank) = vector {
      rrf += 1.0 / (RRF_K + rank as f64);
    }
    if let Some(score) = vector_scores.get(&key).copied() {
      result.vector_score = Some(score);
    }
    result.score = rrf;
  }
}

fn search_mode(token_rank_empty: bool, vector_hits: usize) -> &'static str {
  if vector_hits == 0 {
    "keyword"
  } else if token_rank_empty {
    "vector"
  } else {
    "hybrid"
  }
}

fn normalize_path(path: &str) -> String {
  path.replace('\\', "/")
}

pub(crate) fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f32> {
  if left.is_empty() || left.len() != right.len() {
    return None;
  }

  let mut dot = 0.0f32;
  let mut left_norm = 0.0f32;
  let mut right_norm = 0.0f32;
  for (left_value, right_value) in left.iter().zip(right.iter()) {
    dot += left_value * right_value;
    left_norm += left_value * left_value;
    right_norm += right_value * right_value;
  }

  if left_norm <= 0.0 || right_norm <= 0.0 {
    return None;
  }

  Some(dot / (left_norm.sqrt() * right_norm.sqrt()))
}

#[derive(Debug, Clone)]
struct PageDocument {
  page_id: String,
  relative_path: String,
  title: String,
  content: String,
  content_hash: String,
}

fn load_wiki_pages(root: &Path) -> Result<Vec<PageDocument>, ApiError> {
  let wiki_root = root.join("wiki");
  let mut pages = Vec::new();
  if !wiki_root.exists() {
    return Ok(pages);
  }

  collect_wiki_pages(root, &wiki_root, &mut pages)?;
  Ok(pages)
}

fn collect_wiki_pages(
  project_root: &Path,
  dir: &Path,
  pages: &mut Vec<PageDocument>,
) -> Result<(), ApiError> {
  for entry in fs::read_dir(dir).map_err(|error| ApiError::internal(error.to_string()))? {
    let entry = entry.map_err(|error| ApiError::internal(error.to_string()))?;
    let path = entry.path();
    if entry
      .file_type()
      .map_err(|error| ApiError::internal(error.to_string()))?
      .is_dir()
    {
      collect_wiki_pages(project_root, &path, pages)?;
      continue;
    }
    if path.extension().and_then(|value| value.to_str()) != Some("md") {
      continue;
    }

    let stem = path
      .file_stem()
      .and_then(|value| value.to_str())
      .unwrap_or_default();
    if STRUCTURAL_PAGE_IDS.contains(&stem) {
      continue;
    }

    let content = fs::read_to_string(&path).map_err(|error| ApiError::internal(error.to_string()))?;
    let file_name = path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
    let relative_path = path
      .strip_prefix(project_root)
      .unwrap_or(&path)
      .to_string_lossy()
      .replace('\\', "/");
    pages.push(PageDocument {
      page_id: relative_path.clone(),
      relative_path,
      title: extract_title(&content, file_name),
      content_hash: sha256_hex(&content),
      content,
    });
  }

  Ok(())
}

fn sha256_hex(content: &str) -> String {
  let mut hasher = Sha256::new();
  hasher.update(content.as_bytes());
  format!("{:x}", hasher.finalize())
}

fn map_provider_error(error: ProviderError) -> ApiError {
  if error.retryable() {
    ApiError::internal(error.message().to_string())
  } else {
    ApiError::bad_request(error.message().to_string())
  }
}

#[cfg(test)]
mod embedding_config_tests {
    use super::embedding_config_from_row;

    #[test]
    fn disabled_returns_none() {
        assert!(embedding_config_from_row(false, Some("u".into()), None, Some("m".into()), None).is_none());
    }

    #[test]
    fn enabled_but_incomplete_returns_none() {
        assert!(embedding_config_from_row(true, None, None, Some("m".into()), None).is_none());
        assert!(embedding_config_from_row(true, Some("u".into()), None, None, None).is_none());
    }

    #[test]
    fn enabled_and_complete_returns_config() {
        let cfg = embedding_config_from_row(
            true,
            Some("https://api.example.com".into()),
            Some("k".into()),
            Some("text-embedding-3-small".into()),
            Some(45),
        )
        .expect("config");
        assert_eq!(cfg.model, "text-embedding-3-small");
        assert_eq!(cfg.timeout_seconds, 45);
        assert_eq!(cfg.api_key, "k");
    }
}
