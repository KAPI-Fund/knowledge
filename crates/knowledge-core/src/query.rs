use serde::Serialize;

use crate::search::SearchResult;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryCitation {
  pub path: String,
  pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryAnswer {
  pub answer: String,
  pub context_summary: String,
  pub citations: Vec<QueryCitation>,
}

pub fn answer_from_results(query: &str, results: &[SearchResult]) -> QueryAnswer {
  let citations = results
    .iter()
    .take(1)
    .map(|result| QueryCitation {
      path: result.path.clone(),
      title: result.title.clone(),
    })
    .collect::<Vec<_>>();

  let answer = if let Some(first) = results.first() {
    format!("{query} -> {}", first.title)
  } else {
    format!("{query} -> no matching wiki content")
  };

  let context_summary = results
    .iter()
    .take(3)
    .map(|result| format!("{} ({})", result.path, result.title))
    .collect::<Vec<_>>()
    .join("; ");

  QueryAnswer {
    answer,
    context_summary,
    citations,
  }
}
