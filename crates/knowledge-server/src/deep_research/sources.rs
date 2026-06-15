use std::collections::HashSet;

use futures_util::future::join_all;

use crate::http::error::ApiError;
use crate::web_search::config::WebSearchConfig;
use crate::web_search::provider::web_search;
use crate::web_search::WebSearchResult;

pub const MAX_RESEARCH_SOURCES: usize = 20;
const MAX_PER_QUERY: usize = 5;

#[derive(Debug, Default, Clone)]
pub struct ResearchSources {
  pub results: Vec<WebSearchResult>,
  pub errors: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct SourcesAccumulator {
  results: Vec<WebSearchResult>,
  seen: HashSet<String>,
}

impl SourcesAccumulator {
  pub fn merge(&mut self, batch: Vec<WebSearchResult>) {
    for item in batch {
      if self.results.len() >= MAX_RESEARCH_SOURCES {
        return;
      }
      let key = if item.url.is_empty() {
        format!("{}:{}:{}", item.source, item.title, item.snippet).to_lowercase()
      } else {
        item.url.to_lowercase()
      };
      if self.seen.insert(key) {
        self.results.push(item);
      }
    }
  }

  pub fn into_results(self) -> Vec<WebSearchResult> {
    self.results
  }
}

/// Run all queries in parallel against the configured web-search provider.
/// Dedupe by lowercased URL, fall back to `source:title:snippet` when the
/// URL is empty, cap the merged total at `MAX_RESEARCH_SOURCES`. Each
/// query's error is collected but does not abort the others — matches
/// upstream `Promise.allSettled` semantics at deep-research.ts:107.
pub async fn collect_research_sources(
  queries: &[String],
  config: &WebSearchConfig,
) -> Result<ResearchSources, ApiError> {
  let trimmed = queries
    .iter()
    .map(|query| query.trim().to_string())
    .filter(|query| !query.is_empty())
    .collect::<Vec<_>>();
  if trimmed.is_empty() {
    return Ok(ResearchSources::default());
  }

  let futures = trimmed
    .iter()
    .map(|query| web_search(config, query, MAX_PER_QUERY))
    .collect::<Vec<_>>();
  let results = join_all(futures).await;

  let mut accumulator = SourcesAccumulator::default();
  let mut errors = Vec::new();
  for outcome in results {
    match outcome {
      Ok(items) => accumulator.merge(items),
      Err(error) => errors.push(error.to_string()),
    }
  }

  Ok(ResearchSources {
    results: accumulator.into_results(),
    errors,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::web_search::WebSearchResult;

  fn r(title: &str, url: &str, snippet: &str) -> WebSearchResult {
    WebSearchResult {
      title: title.to_string(),
      url: url.to_string(),
      snippet: snippet.to_string(),
      source: "example.com".to_string(),
    }
  }

  #[test]
  fn dedupe_dedups_by_lowercased_url() {
    let mut accumulator = SourcesAccumulator::default();
    accumulator.merge(vec![r("A", "https://example.com/A", "x")]);
    accumulator.merge(vec![r("A2", "https://EXAMPLE.com/a", "y")]);
    let merged = accumulator.into_results();
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].title, "A");
  }

  #[test]
  fn dedupe_falls_back_to_source_title_snippet_when_no_url() {
    let mut accumulator = SourcesAccumulator::default();
    accumulator.merge(vec![WebSearchResult {
      title: "T".to_string(),
      url: String::new(),
      snippet: "s".to_string(),
      source: "engine-a".to_string(),
    }]);
    accumulator.merge(vec![WebSearchResult {
      title: "T".to_string(),
      url: String::new(),
      snippet: "s".to_string(),
      source: "engine-a".to_string(),
    }]);
    assert_eq!(accumulator.into_results().len(), 1);
  }

  #[test]
  fn dedupe_caps_at_max_research_sources() {
    let mut accumulator = SourcesAccumulator::default();
    let many = (0..50)
      .map(|i| r(&format!("T{i}"), &format!("https://example.com/{i}"), "s"))
      .collect::<Vec<_>>();
    accumulator.merge(many);
    assert_eq!(accumulator.into_results().len(), MAX_RESEARCH_SOURCES);
  }
}
