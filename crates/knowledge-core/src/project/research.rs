use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct RenderResearchReference {
  pub title: String,
  pub url: String,
  pub source: String,
}

#[derive(Debug, Clone)]
pub struct RenderResearchPageInput {
  pub topic: String,
  pub slug: String,
  pub date: String,
  pub synthesis: String,
  pub references: Vec<RenderResearchReference>,
}

/// Ported from deep-research.ts:279-283. Strips `<think>...</think>` and
/// `<thinking>...</thinking>` blocks, case-insensitive, including an
/// unclosed block at the end (model truncated mid-thought).
pub fn clean_synthesis_thinking(raw: &str) -> String {
  static CLOSED: OnceLock<Regex> = OnceLock::new();
  static OPEN_END: OnceLock<Regex> = OnceLock::new();

  let closed = CLOSED.get_or_init(|| {
    Regex::new(r"(?is)<think(?:ing)?>\s*.*?</think(?:ing)?>\s*").unwrap()
  });
  let open_end = OPEN_END.get_or_init(|| {
    Regex::new(r"(?is)<think(?:ing)?>\s*.*$").unwrap()
  });

  let after_closed = closed.replace_all(raw, "").to_string();
  let after_open = open_end.replace_all(&after_closed, "").to_string();
  after_open.trim_start().to_string()
}

/// Ported from deep-research.ts:285-302. The frontmatter shape, body, and
/// references list match upstream verbatim.
pub fn render_research_page(input: &RenderResearchPageInput) -> String {
  let title_escaped = input.topic.replace('"', "\\\"");
  let references = if input.references.is_empty() {
    String::new()
  } else {
    input
      .references
      .iter()
      .enumerate()
      .map(|(index, reference)| {
        format!(
          "{}. [{}]({}) — {}",
          index + 1,
          reference.title,
          reference.url,
          reference.source
        )
      })
      .collect::<Vec<_>>()
      .join("\n")
  };
  let cleaned = clean_synthesis_thinking(&input.synthesis);

  format!(
    "---\ntype: query\ntitle: \"Research: {title}\"\ncreated: {date}\norigin: deep-research\ntags: [research]\n---\n\n# Research: {topic}\n\n{body}\n\n## References\n\n{refs}\n",
    title = title_escaped,
    date = input.date,
    topic = input.topic,
    body = cleaned,
    refs = references
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn clean_synthesis_thinking_strips_closed_blocks() {
    let raw = "<think>internal monologue</think>\n\nActual answer.";
    assert_eq!(clean_synthesis_thinking(raw), "Actual answer.");
  }

  #[test]
  fn clean_synthesis_thinking_strips_thinking_blocks_case_insensitive() {
    let raw = "<Thinking>plan</Thinking>\n\nThe answer.";
    assert_eq!(clean_synthesis_thinking(raw), "The answer.");
  }

  #[test]
  fn clean_synthesis_thinking_strips_unclosed_block_at_end() {
    let raw = "Real content.\n\n<think>tail thoughts that never closed";
    assert_eq!(clean_synthesis_thinking(raw), "Real content.\n\n");
  }

  #[test]
  fn render_research_page_emits_upstream_frontmatter_and_references() {
    let input = RenderResearchPageInput {
      topic: "Knowledge Graphs".to_string(),
      slug: "knowledge-graphs".to_string(),
      date: "2026-06-15".to_string(),
      synthesis: "Synthesized text".to_string(),
      references: vec![
        RenderResearchReference {
          title: "Intro".to_string(),
          url: "https://example.com/intro".to_string(),
          source: "example.com".to_string(),
        },
        RenderResearchReference {
          title: "Deep dive".to_string(),
          url: "https://other.example.com/d".to_string(),
          source: "other.example.com".to_string(),
        },
      ],
    };
    let rendered = render_research_page(&input);
    assert!(rendered.starts_with("---\n"));
    assert!(rendered.contains("type: query"));
    assert!(rendered.contains("title: \"Research: Knowledge Graphs\""));
    assert!(rendered.contains("created: 2026-06-15"));
    assert!(rendered.contains("origin: deep-research"));
    assert!(rendered.contains("tags: [research]"));
    assert!(rendered.contains("# Research: Knowledge Graphs"));
    assert!(rendered.contains("Synthesized text"));
    assert!(rendered.contains("## References"));
    assert!(rendered.contains("1. [Intro](https://example.com/intro) — example.com"));
    assert!(rendered.contains("2. [Deep dive](https://other.example.com/d) — other.example.com"));
  }

  #[test]
  fn render_research_page_escapes_double_quotes_in_topic() {
    let input = RenderResearchPageInput {
      topic: r#"What is "RAG"?"#.to_string(),
      slug: "rag".to_string(),
      date: "2026-06-15".to_string(),
      synthesis: "Body.".to_string(),
      references: vec![],
    };
    let rendered = render_research_page(&input);
    assert!(rendered.contains(r#"title: "Research: What is \"RAG\"?""#));
  }
}
