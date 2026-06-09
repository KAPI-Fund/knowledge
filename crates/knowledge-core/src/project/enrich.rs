use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnrichLink {
  pub term: String,
  pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrichPrompt {
  pub system_prompt: String,
  pub user_prompt: String,
}

pub fn build_enrich_prompt(index_content: &str, page_content: &str) -> EnrichPrompt {
  EnrichPrompt {
    system_prompt: [
      "You identify which terms in a wiki page should become [[wikilinks]] pointing to existing wiki pages.",
      "",
      "You will receive:",
      "  - a wiki index listing existing pages",
      "  - the content of ONE wiki page",
      "",
      "Return a JSON object listing which terms in the page content should be linked to which index entries.",
      "",
      "Response format (EXACTLY this JSON shape, nothing else):",
      "{",
      "  \"links\": [",
      "    { \"term\": \"exact text appearing in the content\", \"target\": \"index page name\" }",
      "  ]",
      "}",
      "",
      "Rules:",
      "- Each \"term\" MUST be a literal substring present in the page content (case-sensitive).",
      "- Each \"target\" MUST be a page listed in the wiki index.",
      "- Include at most one entry per target (first mention).",
      "- Only include clearly-matching terms.",
      "- If no terms should be linked, return `{\"links\": []}`.",
      "- Do NOT output preamble, explanations, or markdown fences - ONLY the JSON object.",
      "",
      &format!("## Wiki Index\n{index_content}"),
    ]
    .join("\n"),
    user_prompt: format!("Page content:\n\n{page_content}"),
  }
}

pub fn parse_enrich_response(raw: &str) -> Vec<EnrichLink> {
  let Some(json_text) = extract_json_object(raw) else {
    return Vec::new();
  };

  let Ok(parsed) = serde_json::from_str::<Value>(json_text) else {
    return Vec::new();
  };

  let Some(links) = parsed.get("links").and_then(Value::as_array) else {
    return Vec::new();
  };

  links
    .iter()
    .filter_map(|item| {
      let term = item.get("term").and_then(Value::as_str)?.trim();
      let target = item.get("target").and_then(Value::as_str)?.trim();
      if term.is_empty() || target.is_empty() {
        return None;
      }
      Some(EnrichLink {
        term: term.to_string(),
        target: target.to_string(),
      })
    })
    .collect()
}

pub fn apply_enrich_links(content: &str, links: &[EnrichLink]) -> String {
  let (frontmatter, mut body) = split_frontmatter(content);
  let mut seen_targets = BTreeSet::new();

  for link in links {
    if link.term.trim().is_empty() || link.target.trim().is_empty() {
      continue;
    }
    if !seen_targets.insert(link.target.to_lowercase()) {
      continue;
    }

    let Some(index) = find_unlinked_occurrence(&body, &link.term) else {
      continue;
    };

    let replacement = if link.term.eq_ignore_ascii_case(&link.target) {
      format!("[[{}]]", link.term)
    } else {
      format!("[[{}|{}]]", link.target, link.term)
    };
    body.replace_range(index..index + link.term.len(), &replacement);
  }

  format!("{frontmatter}{body}")
}

pub fn enrich_page_content(content: &str, raw_response: &str) -> String {
  let links = parse_enrich_response(raw_response);
  if links.is_empty() {
    return content.to_string();
  }

  apply_enrich_links(content, &links)
}

fn extract_json_object(raw: &str) -> Option<&str> {
  let text = raw.trim();
  let text = if let Some(rest) = text.strip_prefix("```json") {
    rest.trim_start()
  } else if let Some(rest) = text.strip_prefix("```") {
    rest.trim_start()
  } else {
    text
  };
  let text = if let Some(rest) = text.strip_suffix("```") {
    rest.trim_end()
  } else {
    text
  };

  let start = text.find('{')?;
  let mut depth = 0usize;
  let mut in_string = false;
  let mut escape = false;
  let mut end = None;

  for (offset, ch) in text[start..].char_indices() {
    if escape {
      escape = false;
      continue;
    }
    if in_string && ch == '\\' {
      escape = true;
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
    } else if ch == '}' {
      depth = depth.saturating_sub(1);
      if depth == 0 {
        end = Some(start + offset + ch.len_utf8());
        break;
      }
    }
  }

  end.map(|end| &text[start..end])
}

fn split_frontmatter(content: &str) -> (String, String) {
  if !content.starts_with("---\n") {
    return (String::new(), content.to_string());
  }

  let Some(end) = content[4..].find("\n---\n") else {
    return (String::new(), content.to_string());
  };

  let closing = 4 + end + "\n---\n".len();
  (content[..closing].to_string(), content[closing..].to_string())
}

fn find_unlinked_occurrence(text: &str, term: &str) -> Option<usize> {
  let mut search_from = 0usize;
  while search_from < text.len() {
    let slice = &text[search_from..];
    let index = slice.find(term)? + search_from;
    if index >= 2 && &text[index - 2..index] == "[[" {
      search_from = index + term.len();
      continue;
    }
    return Some(index);
  }

  None
}

#[cfg(test)]
mod tests {
  use super::{apply_enrich_links, build_enrich_prompt, parse_enrich_response, EnrichLink};

  #[test]
  fn parses_and_applies_enrich_links() {
    let parsed = parse_enrich_response(
      "```json\n{\"links\":[{\"term\":\"Attention\",\"target\":\"attention\"},{\"term\":\"Transformer\",\"target\":\"transformer\"}]}\n```",
    );
    assert_eq!(
      parsed,
      vec![
        EnrichLink {
          term: "Attention".to_string(),
          target: "attention".to_string(),
        },
        EnrichLink {
          term: "Transformer".to_string(),
          target: "transformer".to_string(),
        },
      ]
    );

    let content = "---\ntitle: Attention\n---\n\nAttention uses Transformer. Attention remains important.\n";
    let enriched = apply_enrich_links(content, &parsed);
    assert!(enriched.starts_with("---\ntitle: Attention\n---\n\n"));
    assert!(enriched.contains("[[attention]] uses [[transformer]]") || enriched.contains("[[Attention]] uses [[Transformer]]"));
    assert!(enriched.contains("Attention remains important."));
  }

  #[test]
  fn builds_prompt_with_index_and_page_content() {
    let prompt = build_enrich_prompt("# Index\n- [[attention]]", "Transformer uses attention.");
    assert!(prompt.system_prompt.contains("Wiki Index"));
    assert!(prompt.system_prompt.contains("[[attention]]"));
    assert!(prompt.user_prompt.contains("Transformer uses attention."));
  }
}
