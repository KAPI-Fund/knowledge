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

  QueryAnswer { answer, citations }
}
