//! Graph-relevance ranking ported from upstream_llm_wiki
//! src/lib/graph-relevance.ts (weights and signals preserved verbatim).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

type RawNode = (String, String, String, String, Vec<String>, Vec<String>);

const DIRECT_LINK_WEIGHT: f64 = 3.0;
const SOURCE_OVERLAP_WEIGHT: f64 = 4.0;
const COMMON_NEIGHBOR_WEIGHT: f64 = 1.5;
const TYPE_AFFINITY_WEIGHT: f64 = 1.0;

#[derive(Debug, Clone)]
pub struct RetrievalNode {
    pub id: String,
    pub title: String,
    pub node_type: String,
    pub relative_path: String,
    pub sources: Vec<String>,
    pub out_links: BTreeSet<String>,
    pub in_links: BTreeSet<String>,
}

#[derive(Debug, Default)]
pub struct RetrievalGraph {
    pub nodes: BTreeMap<String, RetrievalNode>,
}

pub fn build_retrieval_graph(wiki_root: &Path) -> RetrievalGraph {
    let mut raw_nodes: Vec<RawNode> = Vec::new();
    collect_markdown(wiki_root, wiki_root, &mut raw_nodes);

    let node_ids: BTreeSet<String> = raw_nodes.iter().map(|(id, ..)| id.clone()).collect();
    let mut out_links: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut in_links: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for (id, _, _, _, _, raw_links) in &raw_nodes {
        for raw in raw_links {
            let Some(resolved) = resolve_target(raw, &node_ids) else {
                continue;
            };
            if &resolved == id {
                continue;
            }
            out_links
                .entry(id.clone())
                .or_default()
                .insert(resolved.clone());
            in_links.entry(resolved).or_default().insert(id.clone());
        }
    }

    let mut nodes = BTreeMap::new();
    for (id, title, node_type, relative_path, sources, _) in raw_nodes {
        nodes.insert(
            id.clone(),
            RetrievalNode {
                out_links: out_links.remove(&id).unwrap_or_default(),
                in_links: in_links.remove(&id).unwrap_or_default(),
                id,
                title,
                node_type,
                relative_path,
                sources,
            },
        );
    }

    RetrievalGraph { nodes }
}

fn collect_markdown(wiki_root: &Path, dir: &Path, raw_nodes: &mut Vec<RawNode>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(wiki_root, &path, raw_nodes);
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".md") {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let id = name.trim_end_matches(".md").to_string();
        let relative_path = path
            .strip_prefix(wiki_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let (title, node_type, sources) = extract_frontmatter(&content, &id);
        let links = extract_wikilinks(&content);
        raw_nodes.push((id, title, node_type, relative_path, sources, links));
    }
}

fn extract_frontmatter(content: &str, fallback_id: &str) -> (String, String, Vec<String>) {
    let mut title = String::new();
    let mut node_type = String::from("other");
    let mut sources = Vec::new();

    if let Some(rest) = content.strip_prefix("---\n")
        && let Some(end) = rest.find("\n---")
    {
        let mut in_sources_block = false;
        for line in rest[..end].lines() {
            if in_sources_block {
                if let Some(item) = line.trim().strip_prefix("- ") {
                    sources.push(item.trim().trim_matches(['"', '\'']).to_string());
                    continue;
                }
                in_sources_block = false;
            }
            if let Some(value) = line.strip_prefix("title:") {
                title = value.trim().trim_matches(['"', '\'']).to_string();
            } else if let Some(value) = line.strip_prefix("type:") {
                node_type = value.trim().trim_matches(['"', '\'']).to_lowercase();
            } else if let Some(value) = line.strip_prefix("sources:") {
                let value = value.trim();
                if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                    sources.extend(
                        inner
                            .split(',')
                            .map(|item| item.trim().trim_matches(['"', '\'']).to_string())
                            .filter(|item| !item.is_empty()),
                    );
                } else if value.is_empty() {
                    in_sources_block = true;
                }
            }
        }
    }

    if title.is_empty() {
        title = content
            .lines()
            .find_map(|line| line.strip_prefix("# ").map(|h| h.trim().to_string()))
            .unwrap_or_else(|| fallback_id.replace('-', " "));
    }

    (title, node_type, sources)
}

fn extract_wikilinks(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("]]") else {
            break;
        };
        let inner = &rest[..end];
        let target = inner.split('|').next().unwrap_or(inner).trim();
        if !target.is_empty() {
            links.push(target.to_string());
        }
        rest = &rest[end + 2..];
    }
    links
}

fn resolve_target(raw: &str, node_ids: &BTreeSet<String>) -> Option<String> {
    if node_ids.contains(raw) {
        return Some(raw.to_string());
    }
    let raw_lower = raw.to_lowercase();
    let normalized = raw_lower.split_whitespace().collect::<Vec<_>>().join("-");
    for id in node_ids {
        let id_lower = id.to_lowercase();
        if id_lower == normalized || id_lower == raw_lower {
            return Some(id.clone());
        }
        if id_lower.split_whitespace().collect::<Vec<_>>().join("-") == normalized {
            return Some(id.clone());
        }
    }
    None
}

pub fn calculate_relevance(a: &RetrievalNode, b: &RetrievalNode, graph: &RetrievalGraph) -> f64 {
    if a.id == b.id {
        return 0.0;
    }

    let forward = if a.out_links.contains(&b.id) { 1.0 } else { 0.0 };
    let backward = if b.out_links.contains(&a.id) { 1.0 } else { 0.0 };
    let direct_link_score = (forward + backward) * DIRECT_LINK_WEIGHT;

    let sources_a: BTreeSet<&String> = a.sources.iter().collect();
    let shared = b.sources.iter().filter(|s| sources_a.contains(s)).count() as f64;
    let source_overlap_score = shared * SOURCE_OVERLAP_WEIGHT;

    let neighbors_a: BTreeSet<&String> = a.out_links.iter().chain(a.in_links.iter()).collect();
    let neighbors_b: BTreeSet<&String> = b.out_links.iter().chain(b.in_links.iter()).collect();
    let mut adamic_adar = 0.0;
    for neighbor_id in neighbors_a.intersection(&neighbors_b) {
        if let Some(neighbor) = graph.nodes.get(neighbor_id.as_str()) {
            let degree = (neighbor.out_links.len() + neighbor.in_links.len()).max(2) as f64;
            adamic_adar += 1.0 / degree.ln();
        }
    }
    let common_neighbor_score = adamic_adar * COMMON_NEIGHBOR_WEIGHT;

    let type_affinity_score = type_affinity(&a.node_type, &b.node_type) * TYPE_AFFINITY_WEIGHT;

    direct_link_score + source_overlap_score + common_neighbor_score + type_affinity_score
}

pub fn related_nodes(graph: &RetrievalGraph, node_id: &str, limit: usize) -> Vec<(String, f64)> {
    let Some(source) = graph.nodes.get(node_id) else {
        return Vec::new();
    };
    let mut scored: Vec<(String, f64)> = graph
        .nodes
        .values()
        .filter(|node| node.id != node_id)
        .map(|node| (node.id.clone(), calculate_relevance(source, node, graph)))
        .filter(|(_, score)| *score > 0.0)
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    scored
}

fn type_affinity(a: &str, b: &str) -> f64 {
    match (a, b) {
        ("entity", "concept") => 1.2,
        ("entity", "entity") => 0.8,
        ("entity", "source") => 1.0,
        ("entity", "synthesis") => 1.0,
        ("entity", "query") => 0.8,
        ("concept", "entity") => 1.2,
        ("concept", "concept") => 0.8,
        ("concept", "source") => 1.0,
        ("concept", "synthesis") => 1.2,
        ("concept", "query") => 1.0,
        ("source", "entity") => 1.0,
        ("source", "concept") => 1.0,
        ("source", "source") => 0.5,
        ("source", "query") => 0.8,
        ("source", "synthesis") => 1.0,
        ("query", "concept") => 1.0,
        ("query", "entity") => 0.8,
        ("query", "synthesis") => 1.0,
        ("query", "source") => 0.8,
        ("query", "query") => 0.5,
        ("synthesis", "concept") => 1.2,
        ("synthesis", "entity") => 1.0,
        ("synthesis", "source") => 1.0,
        ("synthesis", "query") => 1.0,
        ("synthesis", "synthesis") => 0.8,
        _ => 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_page(root: &std::path::Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn ranks_directly_linked_and_source_sharing_pages_highest() {
        let temp = tempfile::tempdir().unwrap();
        let wiki = temp.path().join("wiki");
        write_page(
            &wiki,
            "concepts/attention.md",
            "---\ntype: concept\ntitle: Attention\nsources: [\"paper.pdf\"]\n---\n\nUses [[KV Cache]] heavily.\n",
        );
        write_page(
            &wiki,
            "concepts/kv-cache.md",
            "---\ntype: concept\ntitle: KV Cache\nsources: [\"paper.pdf\"]\n---\n\nCaches keys and values.\n",
        );
        write_page(
            &wiki,
            "concepts/tokenizer.md",
            "---\ntype: concept\ntitle: Tokenizer\nsources: [\"other.pdf\"]\n---\n\nUnrelated page.\n",
        );

        let graph = build_retrieval_graph(&wiki);
        assert_eq!(graph.nodes.len(), 3);

        // [[KV Cache]] resolves to kv-cache via space->dash normalization.
        assert!(graph.nodes["attention"].out_links.contains("kv-cache"));
        assert!(graph.nodes["kv-cache"].in_links.contains("attention"));

        let related = related_nodes(&graph, "attention", 5);
        assert_eq!(related[0].0, "kv-cache");
        // Direct link (2-way counted once each direction: 1*3.0) + shared source (1*4.0)
        // dominates the tokenizer's type-affinity-only score.
        assert!(
            related[0].1
                > related
                    .iter()
                    .find(|(id, _)| id == "tokenizer")
                    .map(|(_, s)| *s)
                    .unwrap_or(0.0)
        );
    }

    #[test]
    fn type_affinity_defaults_to_half_for_unknown_pairs() {
        assert_eq!(type_affinity("concept", "synthesis"), 1.2);
        assert_eq!(type_affinity("source", "source"), 0.5);
        assert_eq!(type_affinity("other", "concept"), 0.5);
    }
}
