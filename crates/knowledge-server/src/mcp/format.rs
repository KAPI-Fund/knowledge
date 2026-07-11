//! Text formatters for MCP tool results, ported from
//! upstream_llm_wiki/mcp-server/src/index.ts L302-450.

use knowledge_core::graph::{GraphEdge, GraphNode};
use knowledge_core::project::files::ProjectFileNode;
use knowledge_core::project::reviews::ReviewItem;
use knowledge_core::search::ProjectSearchResponse;

use crate::agent::events::AgentEvent;
use crate::agent::types::AgentReference;

pub const MAX_TEXT_BYTES: usize = 120_000;

pub fn truncate_text(value: &str, max_bytes: usize) -> String {
  let bytes = value.len();
  if bytes <= max_bytes {
    return value.to_string();
  }
  let mut out = String::new();
  let mut used = 0usize;
  for ch in value.chars() {
    let size = ch.len_utf8();
    if used + size > max_bytes {
      break;
    }
    out.push(ch);
    used += size;
  }
  format!("{out}\n\n[truncated: {} bytes omitted]", bytes - used)
}

pub fn format_file_tree(files: &[ProjectFileNode], truncated: bool) -> String {
  if files.is_empty() {
    return "No files found.".to_string();
  }
  let mut lines: Vec<String> = if truncated {
    vec![
      "[warning] File tree was truncated by the maxFiles limit.".to_string(),
      String::new(),
    ]
  } else {
    Vec::new()
  };
  fn walk(nodes: &[ProjectFileNode], depth: usize, lines: &mut Vec<String>) {
    for node in nodes {
      let prefix = "  ".repeat(depth);
      let icon = if node.is_dir { "📁" } else { "📄" };
      lines.push(format!("{prefix}{icon} {}", node.path));
      if let Some(children) = &node.children {
        walk(children, depth + 1, lines);
      }
    }
  }
  walk(files, 0, &mut lines);
  lines.join("\n")
}

pub fn format_search_results(query: &str, search: &ProjectSearchResponse) -> String {
  if search.results.is_empty() {
    return format!("No results for \"{query}\".");
  }
  let meta = format!(
    "Mode: {} | Token hits: {} | Vector hits: {} | Graph hits: {}",
    search.mode, search.token_hits, search.vector_hits, search.graph_hits
  );
  let mut lines = vec![format!("# Search results for \"{query}\""), meta, String::new()];
  for (index, result) in search.results.iter().enumerate() {
    lines.push(format!("## {}. {}", index + 1, result.title));
    lines.push(format!("Path: {}", result.path));
    let vector = result
      .vector_score
      .map(|score| format!(" | Vector score: {score:.6}"))
      .unwrap_or_default();
    lines.push(format!("Score: {:.6}{vector}", result.score));
    if !result.snippet.is_empty() {
      lines.push(format!("Snippet: {}", result.snippet));
    }
    if !result.images.is_empty() {
      let urls = result
        .images
        .iter()
        .map(|image| image.url.as_str())
        .collect::<Vec<_>>()
        .join(", ");
      lines.push(format!("Images: {urls}"));
    }
    lines.push(String::new());
  }
  lines.join("\n")
}

pub fn format_chat_response(
  message: &str,
  references: &[AgentReference],
  events: &[AgentEvent],
  conversation_id: Option<&str>,
  mode: &str,
  project_id: &str,
) -> String {
  let mut lines = vec![
    "# Knowledge Agent response".to_string(),
    String::new(),
    format!("Session: {}", conversation_id.unwrap_or("(none)")),
    format!("Mode: {mode}"),
    format!("Project: {project_id}"),
    String::new(),
    if message.is_empty() {
      "(empty response)".to_string()
    } else {
      message.to_string()
    },
    String::new(),
  ];

  if !references.is_empty() {
    lines.push("## References".to_string());
    for (index, reference) in references.iter().enumerate() {
      let title = if reference.title.is_empty() { &reference.path } else { &reference.title };
      lines.push(format!("{}. {title}", index + 1));
      lines.push(format!("   Kind: {}", reference.kind));
      lines.push(format!("   Path: {}", reference.path));
      if let Some(score) = reference.score {
        lines.push(format!("   Score: {score:.6}"));
      }
      if let Some(snippet) = &reference.snippet {
        lines.push(format!("   Snippet: {snippet}"));
      }
    }
    lines.push(String::new());
  }

  let tool_events = events
    .iter()
    .filter_map(|event| match event {
      AgentEvent::ToolEnd { tool, .. } => Some(format!("- {tool}: done")),
      _ => None,
    })
    .collect::<Vec<_>>();
  if !tool_events.is_empty() {
    lines.push("## Tool events".to_string());
    lines.extend(tool_events);
  }

  lines.join("\n")
}

pub fn format_reviews(reviews: &[ReviewItem], status: &str) -> String {
  if reviews.is_empty() {
    return format!("No {status} review items found.");
  }
  let mut lines = vec![
    "# Review items".to_string(),
    String::new(),
    format!("Status: {status}"),
    format!("Count: {}", reviews.len()),
    String::new(),
  ];
  for (index, review) in reviews.iter().enumerate() {
    let title = if review.title.is_empty() { &review.id } else { &review.title };
    lines.push(format!("## {}. {title}", index + 1));
    lines.push(format!("ID: {}", review.id));
    lines.push(format!("Type: {}", review.review_type));
    lines.push(format!(
      "Resolved: {}",
      if review.status == "resolved" { "yes" } else { "no" }
    ));
    if let Some(source_path) = &review.source_path {
      lines.push(format!("Source: {source_path}"));
    }
    if let Some(pages) = &review.affected_pages
      && !pages.is_empty()
    {
      lines.push(format!("Affected pages: {}", pages.join(", ")));
    }
    if let Some(queries) = &review.search_queries
      && !queries.is_empty()
    {
      lines.push(format!("Search queries: {}", queries.join(", ")));
    }
    if !review.description.is_empty() {
      lines.push(format!("Description: {}", review.description));
    }
    if !review.options.is_empty() {
      let summary = review
        .options
        .iter()
        .map(|option| {
          if option.label.is_empty() {
            option.action.clone()
          } else {
            format!("{} ({})", option.label, option.action)
          }
        })
        .collect::<Vec<_>>()
        .join(", ");
      lines.push(format!("Options: {summary}"));
    }
    lines.push(String::new());
  }
  lines.join("\n")
}

pub fn format_graph(nodes: &[GraphNode], edges: &[GraphEdge]) -> String {
  let mut type_counts = std::collections::BTreeMap::<&str, usize>::new();
  for node in nodes {
    *type_counts.entry(node.node_type.as_str()).or_insert(0) += 1;
  }
  let mut counts = type_counts.into_iter().collect::<Vec<_>>();
  counts.sort_by(|a, b| b.1.cmp(&a.1));

  let mut top = nodes.iter().collect::<Vec<_>>();
  top.sort_by(|a, b| b.link_count.cmp(&a.link_count));

  let mut lines = vec![
    "# Knowledge graph".to_string(),
    String::new(),
    format!("Nodes: {}", nodes.len()),
    format!("Edges: {}", edges.len()),
    String::new(),
    "## Node types".to_string(),
  ];
  lines.extend(counts.iter().map(|(node_type, count)| format!("- {node_type}: {count}")));
  lines.push(String::new());
  lines.push("## Top nodes".to_string());
  lines.extend(top.iter().take(30).map(|node| {
    let path = if node.path.is_empty() {
      String::new()
    } else {
      format!(" — {}", node.path)
    };
    format!("- {} ({}, {} links){path}", node.label, node.node_type, node.link_count)
  }));
  lines.join("\n")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn truncate_text_returns_input_under_limit() {
    assert_eq!(truncate_text("hello", 100), "hello");
  }

  #[test]
  fn truncate_text_cuts_on_utf8_char_boundary_and_reports_omitted_bytes() {
    // "你" is 3 bytes; limit 4 keeps only one char.
    let out = truncate_text("你好", 4);
    assert!(out.starts_with("你\n\n[truncated: 3 bytes omitted]"), "{out}");
  }

  #[test]
  fn format_file_tree_handles_empty_and_nested() {
    assert_eq!(format_file_tree(&[], false), "No files found.");
    let files = vec![ProjectFileNode {
      name: "wiki".to_string(),
      path: "wiki".to_string(),
      is_dir: true,
      size: None,
      children: Some(vec![ProjectFileNode {
        name: "index.md".to_string(),
        path: "wiki/index.md".to_string(),
        is_dir: false,
        size: Some(10),
        children: None,
      }]),
    }];
    let out = format_file_tree(&files, true);
    assert!(out.contains("[warning] File tree was truncated"));
    assert!(out.contains("📁 wiki"));
    assert!(out.contains("  📄 wiki/index.md"));
  }

  #[test]
  fn format_search_results_reports_empty_and_scores() {
    use knowledge_core::search::{SearchImageRef, SearchResult};
    let empty = ProjectSearchResponse {
      mode: "keyword".to_string(),
      results: Vec::new(),
      token_hits: 0,
      vector_hits: 0,
      graph_hits: 0,
    };
    assert_eq!(format_search_results("q", &empty), "No results for \"q\".");

    let response = ProjectSearchResponse {
      mode: "hybrid".to_string(),
      results: vec![SearchResult {
        path: "wiki/a.md".to_string(),
        title: "A".to_string(),
        snippet: "snip".to_string(),
        title_match: true,
        score: 1.5,
        vector_score: Some(0.25),
        images: vec![SearchImageRef {
          url: "wiki/media/a.png".to_string(),
          alt: String::new(),
        }],
        content: None,
        graph_related_to: Vec::new(),
      }],
      token_hits: 1,
      vector_hits: 1,
      graph_hits: 0,
    };
    let out = format_search_results("alpha", &response);
    assert!(out.contains("# Search results for \"alpha\""));
    assert!(out.contains("Mode: hybrid | Token hits: 1 | Vector hits: 1 | Graph hits: 0"));
    assert!(out.contains("## 1. A"));
    assert!(out.contains("Score: 1.500000 | Vector score: 0.250000"));
    assert!(out.contains("Images: wiki/media/a.png"));
  }

  #[test]
  fn format_chat_response_includes_references_and_tool_events() {
    let references = vec![AgentReference {
      title: "Page".to_string(),
      path: "wiki/page.md".to_string(),
      kind: "wiki".to_string(),
      snippet: Some("snippet".to_string()),
      score: Some(0.5),
    }];
    let events = vec![
      AgentEvent::tool_start("wiki.search", None),
      AgentEvent::tool_end("wiki.search", Some("3 results".to_string())),
    ];
    let out = format_chat_response(
      "answer",
      &references,
      &events,
      Some("conv-1"),
      "standard",
      "project-1",
    );
    assert!(out.contains("Session: conv-1"));
    assert!(out.contains("Mode: standard"));
    assert!(out.contains("Project: project-1"));
    assert!(out.contains("## References"));
    assert!(out.contains("1. Page"));
    assert!(out.contains("   Score: 0.500000"));
    assert!(out.contains("## Tool events"));
    assert!(out.contains("- wiki.search: done"));
  }

  #[test]
  fn format_chat_response_marks_one_shot_sessions_and_empty_message() {
    let out = format_chat_response("", &[], &[], None, "fast", "p");
    assert!(out.contains("Session: (none)"));
    assert!(out.contains("(empty response)"));
    assert!(!out.contains("## References"));
    assert!(!out.contains("## Tool events"));
  }

  #[test]
  fn format_reviews_reports_empty_and_items() {
    use knowledge_core::project::reviews::ReviewOption;
    assert_eq!(format_reviews(&[], "unresolved"), "No unresolved review items found.");
    let reviews = vec![ReviewItem {
      id: "r1".to_string(),
      status: "open".to_string(),
      review_type: "suggestion".to_string(),
      title: "Check source".to_string(),
      description: "desc".to_string(),
      source_path: Some("raw/sources/a.md".to_string()),
      affected_pages: Some(vec!["wiki/a.md".to_string()]),
      search_queries: None,
      options: vec![ReviewOption {
        label: "Resolve".to_string(),
        action: "resolve".to_string(),
      }],
    }];
    let out = format_reviews(&reviews, "unresolved");
    assert!(out.contains("Status: unresolved"));
    assert!(out.contains("Count: 1"));
    assert!(out.contains("## 1. Check source"));
    assert!(out.contains("Resolved: no"));
    assert!(out.contains("Options: Resolve (resolve)"));
  }

  #[test]
  fn format_graph_counts_types_and_ranks_by_links() {
    let nodes = vec![
      GraphNode {
        id: "a".to_string(),
        label: "A".to_string(),
        node_type: "page".to_string(),
        path: "wiki/a.md".to_string(),
        link_count: 1,
        sources: Vec::new(),
      },
      GraphNode {
        id: "b".to_string(),
        label: "B".to_string(),
        node_type: "page".to_string(),
        path: "wiki/b.md".to_string(),
        link_count: 5,
        sources: Vec::new(),
      },
    ];
    let edges = vec![GraphEdge {
      source: "a".to_string(),
      target: "b".to_string(),
      weight: 1.0,
    }];
    let out = format_graph(&nodes, &edges);
    assert!(out.contains("Nodes: 2"));
    assert!(out.contains("Edges: 1"));
    assert!(out.contains("- page: 2"));
    let b_pos = out.find("- B (page, 5 links)").expect("B listed");
    let a_pos = out.find("- A (page, 1 links)").expect("A listed");
    assert!(b_pos < a_pos, "higher link count must rank first");
  }
}
