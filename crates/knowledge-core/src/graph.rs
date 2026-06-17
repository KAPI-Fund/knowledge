use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;

const MAX_GRAPH_LIMIT: usize = 1000;
pub(crate) const STRUCTURAL_IDS: &[&str] = &["index", "log", "overview", "schema", "purpose"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
  pub id: String,
  pub label: String,
  pub node_type: String,
  pub path: String,
  pub link_count: usize,
  pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
  pub source: String,
  pub target: String,
  pub weight: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNeighborhood {
  pub node: GraphNode,
  pub neighbors: Vec<GraphNode>,
}

#[derive(Debug, Clone)]
struct PageRecord {
  id: String,
  label: String,
  node_type: String,
  path: String,
  links: Vec<String>,
  sources: Vec<String>,
}

pub fn build_graph(project_root: &Path) -> Result<(Vec<GraphNode>, Vec<GraphEdge>), io::Error> {
  build_graph_view(project_root, None, None, None)
}

pub fn build_graph_view(
  project_root: &Path,
  query: Option<&str>,
  node_type: Option<&str>,
  limit: Option<usize>,
) -> Result<(Vec<GraphNode>, Vec<GraphEdge>), io::Error> {
  let wiki_root = project_root.join("wiki");
  let mut pages = Vec::new();
  collect_pages(&wiki_root, project_root, &mut pages)?;
  Ok(materialize_graph(pages, query, node_type, limit))
}

pub fn neighbors_for_node(
  project_root: &Path,
  node_id: &str,
) -> Result<GraphNeighborhood, io::Error> {
  let (nodes, edges) = build_graph(project_root)?;
  let node = nodes
    .iter()
    .find(|candidate| candidate.id == node_id)
    .cloned()
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "node not found"))?;

  let neighbor_ids = edges
    .iter()
    .filter_map(|edge| {
      if edge.source == node_id {
        Some(edge.target.clone())
      } else if edge.target == node_id {
        Some(edge.source.clone())
      } else {
        None
      }
    })
    .collect::<BTreeSet<_>>();

  let neighbors = nodes
    .into_iter()
    .filter(|candidate| neighbor_ids.contains(&candidate.id))
    .collect::<Vec<_>>();

  Ok(GraphNeighborhood { node, neighbors })
}

fn materialize_graph(
  pages: Vec<PageRecord>,
  query: Option<&str>,
  node_type: Option<&str>,
  limit: Option<usize>,
) -> (Vec<GraphNode>, Vec<GraphEdge>) {
  let aliases = pages
    .iter()
    .map(|page| (normalize_target(&page.id), page.id.clone()))
    .collect::<BTreeMap<_, _>>();

  let mut link_counts = pages
    .iter()
    .map(|page| (page.id.clone(), 0usize))
    .collect::<BTreeMap<_, _>>();
  let mut edge_weights = BTreeMap::<(String, String), f64>::new();

  for page in &pages {
    for raw_target in &page.links {
      let Some(target_id) = resolve_target(raw_target, &aliases) else {
        continue;
      };
      if target_id == page.id {
        continue;
      }

      if let Some(source_count) = link_counts.get_mut(&page.id) {
        *source_count += 1;
      }
      if let Some(target_count) = link_counts.get_mut(&target_id) {
        *target_count += 1;
      }

      let key = ordered_edge_key(&page.id, &target_id);
      *edge_weights.entry(key).or_insert(0.0) += 1.0;
    }
  }

  let mut nodes = pages
    .into_iter()
    .map(|page| GraphNode {
      id: page.id.clone(),
      label: page.label,
      node_type: page.node_type,
      path: page.path,
      link_count: *link_counts.get(&page.id).unwrap_or(&0),
      sources: page.sources,
    })
    .collect::<Vec<_>>();
  let mut edges = edge_weights
    .into_iter()
    .map(|((source, target), weight)| GraphEdge {
      source,
      target,
      weight,
    })
    .collect::<Vec<_>>();

  if let Some(trimmed_query) = query.map(str::trim).filter(|value| !value.is_empty()) {
    let query_tokens = tokenize_query(trimmed_query);
    let visible_ids = nodes
      .iter()
      .filter(|node| node_matches_query(node, &query_tokens))
      .map(|node| node.id.clone())
      .collect::<BTreeSet<_>>();
    nodes.retain(|node| visible_ids.contains(&node.id));
    edges.retain(|edge| visible_ids.contains(&edge.source) && visible_ids.contains(&edge.target));
  }

  if let Some(expected_type) = node_type
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_lowercase)
  {
    let visible_ids = nodes
      .iter()
      .filter(|node| node.node_type == expected_type)
      .map(|node| node.id.clone())
      .collect::<BTreeSet<_>>();
    nodes.retain(|node| visible_ids.contains(&node.id));
    edges.retain(|edge| visible_ids.contains(&edge.source) && visible_ids.contains(&edge.target));
  }

  nodes.sort_by(|left, right| {
    right
      .link_count
      .cmp(&left.link_count)
      .then_with(|| left.label.cmp(&right.label))
      .then_with(|| left.id.cmp(&right.id))
  });
  edges.sort_by(|left, right| {
    right
      .weight
      .total_cmp(&left.weight)
      .then_with(|| left.source.cmp(&right.source))
      .then_with(|| left.target.cmp(&right.target))
  });

  let capped_limit = limit.unwrap_or(MAX_GRAPH_LIMIT).min(MAX_GRAPH_LIMIT);
  if nodes.len() > capped_limit {
    nodes.truncate(capped_limit);
    let visible_ids = nodes.iter().map(|node| node.id.clone()).collect::<BTreeSet<_>>();
    edges.retain(|edge| visible_ids.contains(&edge.source) && visible_ids.contains(&edge.target));
  }

  (nodes, edges)
}

fn collect_pages(root: &Path, project_root: &Path, pages: &mut Vec<PageRecord>) -> Result<(), io::Error> {
  if !root.exists() {
    return Ok(());
  }

  for entry in fs::read_dir(root)? {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      collect_pages(&path, project_root, pages)?;
      continue;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
      continue;
    }

    let id = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("").to_string();
    if should_skip_page(&id) {
      continue;
    }

    let content = fs::read_to_string(&path)?;
    let relative_path = path
      .strip_prefix(project_root)
      .unwrap_or(&path)
      .to_string_lossy()
      .replace('\\', "/");

    pages.push(PageRecord {
      label: extract_title(&content, &id),
      node_type: extract_type(&content),
      links: extract_wikilinks(&content),
      sources: extract_sources(&content),
      id,
      path: relative_path,
    });
  }

  Ok(())
}

fn should_skip_page(id: &str) -> bool {
  STRUCTURAL_IDS.contains(&id)
}

fn extract_title(content: &str, id: &str) -> String {
  extract_frontmatter_field(content, "title")
    .or_else(|| {
      content
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim).map(str::to_string))
    })
    .unwrap_or_else(|| id.replace('-', " "))
}

fn extract_type(content: &str) -> String {
  extract_frontmatter_field(content, "type")
    .map(|value| value.to_lowercase())
    .filter(|value| !value.is_empty())
    .unwrap_or_else(|| "other".to_string())
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

/// Parse a `sources:` frontmatter entry. Supports an inline list
/// (`sources: [a, b]`) and a YAML block list (`sources:` followed by
/// `  - item` lines). Ported from upstream graph-relevance.ts extractFrontmatter.
///
/// Only the YAML frontmatter block (between the opening `---` and the next
/// `---` at the top of the file) is scanned — mirroring how
/// `extract_frontmatter_field` bounds its search.  A `sources:` line that
/// appears in the document body is ignored.
fn extract_sources(content: &str) -> Vec<String> {
  // Collect only the lines inside the frontmatter block, consistent with
  // extract_frontmatter_field.
  let mut lines = content.lines();
  if lines.next().map(str::trim) != Some("---") {
    return Vec::new();
  }
  let fm_lines: Vec<&str> = lines
    .take_while(|line| line.trim() != "---")
    .collect();

  let mut in_block = false;
  let mut sources = Vec::new();

  for line in fm_lines {
    if in_block {
      let trimmed = line.trim_start();
      if let Some(item) = trimmed.strip_prefix('-') {
        let value = clean_source_token(item.trim());
        if !value.is_empty() {
          sources.push(value);
        }
        continue;
      }
      // A non-indented, non-`-` line ends the block.
      if !line.starts_with(char::is_whitespace) {
        in_block = false;
      } else {
        continue;
      }
    }

    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("sources:") {
      let rest = rest.trim();
      if rest.is_empty() {
        in_block = true;
      } else if let Some(inner) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        for token in inner.split(',') {
          let value = clean_source_token(token.trim());
          if !value.is_empty() {
            sources.push(value);
          }
        }
      } else {
        let value = clean_source_token(rest);
        if !value.is_empty() {
          sources.push(value);
        }
      }
    }
  }

  sources
}

fn clean_source_token(token: &str) -> String {
  token
    .trim()
    .trim_matches(|c| c == '"' || c == '\'')
    .trim()
    .to_string()
}

fn extract_wikilinks(content: &str) -> Vec<String> {
  let mut remaining = content;
  let mut links = Vec::new();

  while let Some(start) = remaining.find("[[") {
    remaining = &remaining[start + 2..];
    let Some(end) = remaining.find("]]") else {
      break;
    };
    let target = remaining[..end]
      .split('|')
      .next()
      .unwrap_or("")
      .split('#')
      .next()
      .unwrap_or("")
      .trim();
    if !target.is_empty() {
      links.push(target.to_string());
    }
    remaining = &remaining[end + 2..];
  }

  links
}

fn ordered_edge_key(left: &str, right: &str) -> (String, String) {
  if left <= right {
    (left.to_string(), right.to_string())
  } else {
    (right.to_string(), left.to_string())
  }
}

fn resolve_target(raw_target: &str, aliases: &BTreeMap<String, String>) -> Option<String> {
  let normalized = normalize_target(raw_target);
  aliases.get(&normalized).cloned()
}

pub(crate) fn normalize_target(value: &str) -> String {
  let base = value
    .trim()
    .trim_end_matches(".md")
    .replace('\\', "/")
    .rsplit('/')
    .next()
    .unwrap_or(value)
    .trim()
    .to_lowercase();

  let mut normalized = String::new();
  let mut last_dash = false;
  for ch in base.chars() {
    if ch.is_ascii_alphanumeric() {
      normalized.push(ch);
      last_dash = false;
    } else if !last_dash && !normalized.is_empty() {
      normalized.push('-');
      last_dash = true;
    }
  }

  normalized.trim_matches('-').to_string()
}

fn tokenize_query(query: &str) -> Vec<String> {
  query
    .split_whitespace()
    .map(|token| token.trim().to_lowercase())
    .filter(|token| !token.is_empty())
    .collect::<Vec<_>>()
}

fn node_matches_query(node: &GraphNode, tokens: &[String]) -> bool {
  let haystack = format!(
    "{} {} {} {}",
    node.label, node.id, node.node_type, node.path
  )
  .to_lowercase();

  tokens.iter().all(|token| haystack.contains(token))
}

#[cfg(test)]
mod tests {
  use super::extract_sources;

  #[test]
  fn extracts_block_form_sources() {
    let content = "---\ntitle: Demo\nsources:\n  - alpha.pdf\n  - beta.docx\ntype: concept\n---\nBody";
    assert_eq!(
      extract_sources(content),
      vec!["alpha.pdf".to_string(), "beta.docx".to_string()]
    );
  }

  #[test]
  fn extracts_inline_form_sources() {
    let content = "---\nsources: [alpha.pdf, \"beta.docx\"]\n---\nBody";
    assert_eq!(
      extract_sources(content),
      vec!["alpha.pdf".to_string(), "beta.docx".to_string()]
    );
  }

  #[test]
  fn returns_empty_when_no_sources() {
    let content = "---\ntitle: Demo\ntype: concept\n---\nBody";
    assert!(extract_sources(content).is_empty());
  }

  #[test]
  fn ignores_sources_outside_frontmatter() {
    let content = "---\ntitle: Demo\n---\nBody mentions sources: [ignored.pdf]";
    assert!(extract_sources(content).is_empty());
  }
}
