use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Serialize;

const FILENAME_EXACT_BONUS: f64 = 200.0;
const PHRASE_IN_TITLE_BONUS: f64 = 50.0;
const PHRASE_IN_CONTENT_PER_OCCURRENCE: f64 = 20.0;
const MAX_PHRASE_OCCURRENCES: usize = 10;
const TITLE_TOKEN_WEIGHT: f64 = 5.0;
const CONTENT_TOKEN_WEIGHT: f64 = 1.0;
const SNIPPET_CONTEXT: usize = 80;
const DEFAULT_RESULTS: usize = 10;
const MAX_RESULTS: usize = 100;

#[derive(Debug, Clone, Copy)]
pub struct SearchOptions {
  pub top_k: usize,
  pub include_content: bool,
}

impl Default for SearchOptions {
  fn default() -> Self {
    Self {
      top_k: DEFAULT_RESULTS,
      include_content: false,
    }
  }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchImageRef {
  pub url: String,
  pub alt: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
  pub path: String,
  pub title: String,
  pub snippet: String,
  pub title_match: bool,
  pub score: f64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub vector_score: Option<f32>,
  pub images: Vec<SearchImageRef>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub content: Option<String>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub graph_related_to: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSearchResponse {
  pub mode: String,
  pub results: Vec<SearchResult>,
  pub token_hits: usize,
  pub vector_hits: usize,
  pub graph_hits: usize,
}

pub fn search_project(project_root: &Path, query: &str) -> Result<Vec<SearchResult>, std::io::Error> {
  Ok(search_project_with_options(project_root, query, SearchOptions::default())?.results)
}

pub fn search_project_with_options(
  project_root: &Path,
  query: &str,
  options: SearchOptions,
) -> Result<ProjectSearchResponse, std::io::Error> {
  let wiki_root = project_root.join("wiki");
  let mut results = Vec::new();
  let query_tokens = tokenize_query(query);
  let query_phrase = trim_query_punctuation(&query.to_lowercase());
  let top_k = options.top_k.clamp(1, MAX_RESULTS);

  if !wiki_root.exists() {
    return Ok(ProjectSearchResponse {
      mode: "keyword".to_string(),
      results,
      token_hits: 0,
      vector_hits: 0,
      graph_hits: 0,
    });
  }

  walk_markdown(&wiki_root, &mut |path| {
    let content = fs::read_to_string(path)?;
    if let Some(result) = score_file(
      project_root,
      path,
      &content,
      &query_tokens,
      &query_phrase,
      query,
      options.include_content,
    ) {
      results.push(result);
    }

    Ok(())
  })?;

  results.sort_by(|left, right| {
    right
      .score
      .partial_cmp(&left.score)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then(left.path.cmp(&right.path))
  });
  let token_hits = results.len();
  results.truncate(top_k);

  Ok(ProjectSearchResponse {
    mode: "keyword".to_string(),
    results,
    token_hits,
    vector_hits: 0,
    graph_hits: 0,
  })
}

pub fn tokenize_query(query: &str) -> Vec<String> {
  let raw = query
    .to_lowercase()
    .split(is_query_separator)
    .filter(|token| token.chars().count() > 1)
    .filter(|token| !is_stop_word(token))
    .map(str::to_string)
    .collect::<Vec<_>>();

  let mut tokens = Vec::new();
  for token in raw {
    let chars = token.chars().collect::<Vec<_>>();
    let has_cjk = chars.iter().any(|ch| matches!(*ch, '\u{3400}'..='\u{9fff}'));
    if has_cjk && chars.len() > 2 {
      for pair in chars.windows(2) {
        tokens.push(pair.iter().collect());
      }
      for ch in &chars {
        let single = ch.to_string();
        if !is_stop_word(&single) {
          tokens.push(single);
        }
      }
      tokens.push(token);
    } else {
      tokens.push(token);
    }
  }

  tokens.into_iter().collect::<BTreeSet<_>>().into_iter().collect()
}

pub fn extract_title(content: &str, file_name: &str) -> String {
  let has_frontmatter = content.starts_with("---");
  let mut in_frontmatter = has_frontmatter;
  let mut frontmatter_closed = false;

  for line in content.lines().skip(if has_frontmatter { 1 } else { 0 }) {
    let trimmed = line.trim();
    if in_frontmatter && trimmed == "---" {
      in_frontmatter = false;
      frontmatter_closed = true;
      continue;
    }
    if in_frontmatter && trimmed.starts_with("title:") {
      return trimmed
        .trim_start_matches("title:")
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();
    }
    if has_frontmatter && !frontmatter_closed {
      continue;
    }
    if let Some(title) = trimmed.strip_prefix("# ") {
      return title.trim().to_string();
    }
  }

  file_name.trim_end_matches(".md").replace('-', " ")
}

pub fn build_snippet(content: &str, query: &str) -> String {
  let lower = content.to_lowercase();
  let query_lower = query.to_lowercase();
  let byte_index = lower.find(&query_lower).unwrap_or(0);
  let char_positions = content.char_indices().map(|(index, _)| index).collect::<Vec<_>>();
  if char_positions.is_empty() {
    return String::new();
  }
  let match_char = char_positions
    .iter()
    .position(|index| *index >= byte_index)
    .unwrap_or(char_positions.len().saturating_sub(1));
  let query_chars = query.chars().count().max(1);
  let start_char = match_char.saturating_sub(SNIPPET_CONTEXT);
  let end_char = (match_char + query_chars + SNIPPET_CONTEXT).min(char_positions.len());
  let start = char_positions[start_char];
  let end = if end_char < char_positions.len() {
    char_positions[end_char]
  } else {
    content.len()
  };

  let mut snippet = content[start..end].replace('\n', " ");
  if start > 0 {
    snippet = format!("...{snippet}");
  }
  if end < content.len() {
    snippet.push_str("...");
  }
  snippet
}

pub fn extract_image_refs(content: &str) -> Vec<SearchImageRef> {
  let mut images = Vec::new();
  let mut seen = BTreeSet::new();
  let mut remaining = content;

  while let Some(start) = remaining.find("![") {
    remaining = &remaining[start + 2..];
    let Some(alt_end) = remaining.find("](") else {
      break;
    };
    let alt = &remaining[..alt_end];
    remaining = &remaining[alt_end + 2..];
    let Some(url_end) = remaining.find(')') else {
      break;
    };
    let url = &remaining[..url_end];
    if !url.trim().is_empty() && !url.contains(char::is_whitespace) && seen.insert(url.to_string()) {
      images.push(SearchImageRef {
        url: url.to_string(),
        alt: alt.to_string(),
      });
    }
    remaining = &remaining[url_end + 1..];
  }

  images
}

fn walk_markdown(
  root: &Path,
  visit: &mut impl FnMut(&Path) -> Result<(), std::io::Error>,
) -> Result<(), std::io::Error> {
  for entry in fs::read_dir(root)? {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      walk_markdown(&path, visit)?;
    } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
      visit(&path)?;
    }
  }

  Ok(())
}

fn score_file(
  project_root: &Path,
  path: &Path,
  content: &str,
  tokens: &[String],
  query_phrase: &str,
  query: &str,
  include_content: bool,
) -> Option<SearchResult> {
  let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
  let title = extract_title(content, file_name);
  let title_text = format!("{title} {file_name}");
  let title_lower = title_text.to_lowercase();
  let content_lower = content.to_lowercase();
  let stem = file_name.trim_end_matches(".md").to_lowercase();

  let filename_exact = !query_phrase.is_empty() && stem == query_phrase;
  let title_has_phrase = !query_phrase.is_empty() && title_lower.contains(query_phrase);
  let content_phrase_occurrences = count_occurrences(&content_lower, query_phrase).min(MAX_PHRASE_OCCURRENCES);
  let title_token_score = token_match_score(&title_text, tokens);
  let content_token_score = token_match_score(content, tokens);

  if !filename_exact
    && !title_has_phrase
    && content_phrase_occurrences == 0
    && title_token_score == 0
    && content_token_score == 0
  {
    return None;
  }

  let score = (if filename_exact { FILENAME_EXACT_BONUS } else { 0.0 })
    + (if title_has_phrase { PHRASE_IN_TITLE_BONUS } else { 0.0 })
    + content_phrase_occurrences as f64 * PHRASE_IN_CONTENT_PER_OCCURRENCE
    + title_token_score as f64 * TITLE_TOKEN_WEIGHT
    + content_token_score as f64 * CONTENT_TOKEN_WEIGHT;

  let snippet_anchor = if content_phrase_occurrences > 0 && !query_phrase.is_empty() {
    query_phrase.to_string()
  } else {
    tokens
      .iter()
      .find(|token| content_lower.contains(token.as_str()))
      .cloned()
      .unwrap_or_else(|| query.to_string())
  };

  Some(SearchResult {
    path: relative_to_project(project_root, path),
    title,
    snippet: build_snippet(content, &snippet_anchor),
    title_match: filename_exact || title_has_phrase || title_token_score > 0,
    score,
    vector_score: None,
    images: extract_image_refs(content),
    content: include_content.then_some(content.to_string()),
    graph_related_to: Vec::new(),
  })
}

fn is_query_separator(ch: char) -> bool {
  ch.is_whitespace()
    || ch.is_ascii_punctuation()
    || matches!(
      ch,
      '\u{ff0c}'
        | '\u{3002}'
        | '\u{ff1f}'
        | '\u{ff01}'
        | '\u{ff1b}'
        | '\u{ff1a}'
        | '\u{3001}'
        | '\u{201c}'
        | '\u{201d}'
        | '\u{2018}'
        | '\u{2019}'
        | '\u{ff08}'
        | '\u{ff09}'
        | '\u{00b7}'
        | '\u{301c}'
        | '\u{2026}'
    )
}

fn is_stop_word(token: &str) -> bool {
  matches!(
    token,
    "\u{7684}"
      | "\u{662f}"
      | "\u{5728}"
      | "\u{6709}"
      | "\u{548c}"
      | "\u{4e0e}"
      | "\u{5bf9}"
      | "\u{4ec0}\u{4e48}"
      | "\u{4e3a}\u{4ec0}\u{4e48}"
      | "the"
      | "is"
      | "a"
      | "an"
      | "what"
      | "how"
      | "are"
      | "was"
      | "were"
      | "do"
      | "does"
      | "did"
      | "be"
      | "been"
      | "being"
      | "have"
      | "has"
      | "had"
      | "it"
      | "its"
      | "in"
      | "on"
      | "at"
      | "to"
      | "for"
      | "of"
      | "with"
      | "by"
      | "this"
      | "that"
      | "these"
      | "those"
  )
}

fn trim_query_punctuation(value: &str) -> String {
  value.trim_matches(is_query_separator).to_string()
}

fn token_match_score(text: &str, tokens: &[String]) -> usize {
  let lower = text.to_lowercase();
  tokens
    .iter()
    .filter(|token| lower.contains(token.as_str()))
    .count()
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
  if needle.is_empty() {
    return 0;
  }

  haystack.match_indices(needle).count()
}

fn relative_to_project(project_root: &Path, path: &Path) -> String {
  path
    .strip_prefix(project_root)
    .unwrap_or(path)
    .to_string_lossy()
    .replace('\\', "/")
}

#[cfg(test)]
mod tests {
  use std::fs;

  use super::{extract_image_refs, extract_title, search_project, tokenize_query};

  #[test]
  fn extract_title_prefers_frontmatter_title() {
    let content = "---\ntitle: \"Reasoning Models\"\n---\n\n# Fallback Heading\n";
    assert_eq!(extract_title(content, "reasoning-models.md"), "Reasoning Models");
  }

  #[test]
  fn tokenize_query_splits_cjk_into_bigrams() {
    let query = "\u{77e5}\u{8bc6}\u{56fe}\u{8c31}";
    let tokens = tokenize_query(query);

    assert!(tokens.iter().any(|token| token == "\u{77e5}\u{8bc6}"));
    assert!(tokens.iter().any(|token| token == "\u{8bc6}\u{56fe}"));
    assert!(tokens.iter().any(|token| token == "\u{56fe}\u{8c31}"));
    assert!(tokens.iter().any(|token| token == query));
  }

  #[test]
  fn search_prefers_title_and_phrase_match_ranking() {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path();
    fs::create_dir_all(project_root.join("wiki/concepts")).unwrap();

    fs::write(
      project_root.join("wiki/concepts/reasoning-models.md"),
      "---\ntitle: Reasoning Models\n---\n\n# Reasoning Models\n\nA survey of reasoning models.\n",
    )
    .unwrap();
    fs::write(
      project_root.join("wiki/concepts/background.md"),
      "---\ntitle: Background\n---\n\n# Background\n\nThis page mentions reasoning and models separately.\n",
    )
    .unwrap();

    let results = search_project(project_root, "reasoning models").unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].path, "wiki/concepts/reasoning-models.md");
    assert!(results[0].snippet.contains("Reasoning Models"));
    assert!(results[0].score > results[1].score);
  }

  #[test]
  fn extract_image_refs_returns_unique_markdown_images() {
    let images = extract_image_refs(
      "![Trace Diagram](wiki/media/trace-diagram.png)\n![Trace Diagram](wiki/media/trace-diagram.png)\n![Flow](raw/assets/flow.jpg)",
    );

    assert_eq!(images.len(), 2);
    assert_eq!(images[0].url, "wiki/media/trace-diagram.png");
    assert_eq!(images[0].alt, "Trace Diagram");
    assert_eq!(images[1].url, "raw/assets/flow.jpg");
  }
}
