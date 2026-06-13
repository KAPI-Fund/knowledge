use std::collections::{BTreeMap, BTreeSet};

use crate::retrieval::service::cosine_similarity;
use crate::retrieval::store::StoredChunk;

pub const DEDUP_SIMILARITY_THRESHOLD: f32 = 0.80;
pub const DEDUP_MAX_CANDIDATE_PAGES: usize = 40;

/// Mean-embedding per entity/concept page, pairwise cosine, union of pages
/// appearing in any pair at or above the threshold. Sorted, truncated.
pub fn select_dedup_candidate_pages(
  chunks: &[StoredChunk],
  threshold: f32,
  max_pages: usize,
) -> Vec<String> {
  let mut sums: BTreeMap<&str, (Vec<f32>, usize)> = BTreeMap::new();
  for chunk in chunks {
    let path = chunk.relative_path.as_str();
    if !(path.starts_with("wiki/entities/") || path.starts_with("wiki/concepts/")) {
      continue;
    }
    if chunk.embedding.is_empty() {
      continue;
    }
    let entry = sums
      .entry(path)
      .or_insert_with(|| (vec![0.0; chunk.embedding.len()], 0));
    if entry.0.len() != chunk.embedding.len() {
      continue;
    }
    for (slot, value) in entry.0.iter_mut().zip(chunk.embedding.iter()) {
      *slot += value;
    }
    entry.1 += 1;
  }

  let means = sums
    .into_iter()
    .map(|(path, (sum, count))| {
      let mean = sum
        .into_iter()
        .map(|value| value / count as f32)
        .collect::<Vec<_>>();
      (path, mean)
    })
    .collect::<Vec<_>>();

  let mut candidates = BTreeSet::new();
  for (left_index, (left_path, left_mean)) in means.iter().enumerate() {
    for (right_path, right_mean) in means.iter().skip(left_index + 1) {
      if let Some(score) = cosine_similarity(left_mean, right_mean)
        && score >= threshold
      {
        candidates.insert((*left_path).to_string());
        candidates.insert((*right_path).to_string());
      }
    }
  }

  let mut pages = candidates.into_iter().collect::<Vec<_>>();
  pages.truncate(max_pages);
  pages
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::retrieval::store::StoredChunk;

  fn chunk(relative_path: &str, embedding: Vec<f32>) -> StoredChunk {
    StoredChunk {
      page_id: relative_path.to_string(),
      relative_path: relative_path.to_string(),
      title: "t".to_string(),
      embedding_model: "mock-embedding".to_string(),
      chunk_index: 0,
      heading_path: String::new(),
      chunk_text: "text".to_string(),
      content_hash: "hash".to_string(),
      embedding,
    }
  }

  #[test]
  fn selects_pages_with_mutually_similar_embeddings() {
    let chunks = vec![
      chunk("wiki/concepts/attention.md", vec![0.0, 1.0, 0.0]),
      chunk("wiki/concepts/attention-mechanism.md", vec![0.0, 1.0, 0.0]),
      chunk("wiki/entities/rope.md", vec![1.0, 0.0, 0.0]),
    ];
    let pages = select_dedup_candidate_pages(&chunks, 0.8, 40);
    assert_eq!(
      pages,
      vec![
        "wiki/concepts/attention-mechanism.md".to_string(),
        "wiki/concepts/attention.md".to_string()
      ]
    );
  }

  #[test]
  fn ignores_pages_outside_entities_and_concepts() {
    let chunks = vec![
      chunk("wiki/concepts/attention.md", vec![0.0, 1.0, 0.0]),
      chunk("wiki/sources/transformer-paper.md", vec![0.0, 1.0, 0.0]),
    ];
    assert!(select_dedup_candidate_pages(&chunks, 0.8, 40).is_empty());
  }

  #[test]
  fn averages_chunks_per_page_and_respects_max() {
    let chunks = vec![
      chunk("wiki/concepts/a.md", vec![0.0, 1.0, 0.0]),
      chunk("wiki/concepts/a.md", vec![0.0, 1.0, 0.0]),
      chunk("wiki/concepts/b.md", vec![0.0, 1.0, 0.0]),
      chunk("wiki/concepts/c.md", vec![0.0, 1.0, 0.0]),
    ];
    let pages = select_dedup_candidate_pages(&chunks, 0.8, 2);
    assert_eq!(pages.len(), 2);
  }
}
