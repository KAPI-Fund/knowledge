use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
  pub id: String,
  pub label: String,
  pub node_type: String,
  pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
  pub source: String,
  pub target: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNeighborhood {
  pub node: GraphNode,
  pub neighbors: Vec<GraphNode>,
}

pub fn build_graph(project_root: &Path) -> Result<(Vec<GraphNode>, Vec<GraphEdge>), std::io::Error> {
  let wiki_root = project_root.join("wiki");
  let mut nodes = Vec::new();
  let mut links = BTreeMap::<String, Vec<String>>::new();

  collect_pages(&wiki_root, project_root, &mut nodes, &mut links)?;

  let ids = nodes.iter().map(|node| node.id.clone()).collect::<BTreeSet<_>>();
  let mut edges = Vec::new();
  let mut seen = BTreeSet::new();

  for (source, targets) in links {
    for target in targets {
      if !ids.contains(&target) || source == target {
        continue;
      }
      let key = if source < target {
        format!("{source}::{target}")
      } else {
        format!("{target}::{source}")
      };
      if seen.insert(key) {
        edges.push(GraphEdge {
          source: source.clone(),
          target,
        });
      }
    }
  }

  Ok((nodes, edges))
}

pub fn neighbors_for_node(
  project_root: &Path,
  node_id: &str,
) -> Result<GraphNeighborhood, std::io::Error> {
  let (nodes, edges) = build_graph(project_root)?;
  let node = nodes
    .iter()
    .find(|node| node.id == node_id)
    .cloned()
    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "node not found"))?;

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

fn collect_pages(
  root: &Path,
  project_root: &Path,
  nodes: &mut Vec<GraphNode>,
  links: &mut BTreeMap<String, Vec<String>>,
) -> Result<(), std::io::Error> {
  if !root.exists() {
    return Ok(());
  }

  for entry in fs::read_dir(root)? {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      collect_pages(&path, project_root, nodes, links)?;
      continue;
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
      continue;
    }

    let content = fs::read_to_string(&path)?;
    let id = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("").to_string();
    if matches!(id.as_str(), "index" | "log" | "overview") {
      continue;
    }
    let label = content
      .lines()
      .find_map(|line| line.strip_prefix("# ").map(str::trim))
      .unwrap_or(id.as_str())
      .to_string();
    let node_type = content
      .lines()
      .find_map(|line| line.strip_prefix("type:").map(str::trim))
      .unwrap_or("other")
      .to_string();
    let relative = path
      .strip_prefix(project_root)
      .unwrap_or(&path)
      .to_string_lossy()
      .replace('\\', "/");

    nodes.push(GraphNode {
      id: id.clone(),
      label,
      node_type,
      path: relative,
    });
    links.insert(id, extract_wikilinks(&content));
  }

  Ok(())
}

fn extract_wikilinks(content: &str) -> Vec<String> {
  let mut rest = content;
  let mut links = Vec::new();

  while let Some(start) = rest.find("[[") {
    rest = &rest[start + 2..];
    let Some(end) = rest.find("]]") else {
      break;
    };
    let target = rest[..end].split('|').next().unwrap_or("").trim();
    if !target.is_empty() {
      links.push(target.to_string());
    }
    rest = &rest[end + 2..];
  }

  links
}
