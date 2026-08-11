//! Graph-relevance ranking ported from upstream_llm_wiki
//! src/lib/graph-relevance.ts (weights and signals preserved verbatim).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::graph::{STRUCTURAL_IDS, normalize_target};
use crate::search::{SearchResult, extract_image_refs};

type RawNode = (String, String, String, String, Vec<String>, Vec<String>);

const DIRECT_LINK_WEIGHT: f64 = 3.0;
const SOURCE_OVERLAP_WEIGHT: f64 = 4.0;
const COMMON_NEIGHBOR_WEIGHT: f64 = 1.5;
const TYPE_AFFINITY_WEIGHT: f64 = 1.0;

// Graph search channel constants, upstream commands/search.rs L14-26.
const RRF_K: f64 = 60.0;
const MIN_GRAPH_RESULT_RATIO: f64 = 0.15;
const MAX_GRAPH_RESULT_RATIO: f64 = 0.30;
const MAX_GRAPH_SEEDS: usize = 20;

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

    // Wikilinks carry no directory info, so they resolve via normalized
    // stems (upstream graph-relevance semantics); node identity is the path.
    let mut aliases: BTreeMap<String, String> = BTreeMap::new();
    for (id, ..) in &raw_nodes {
        aliases.insert(normalize_target(id), id.clone());
    }

    let mut out_links: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut in_links: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for (id, _, _, _, _, raw_links) in &raw_nodes {
        for raw in raw_links {
            let Some(resolved) = resolve_target(raw, &aliases) else {
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

/// Reserve 15-30% of the final window for one-hop graph expansion. A full
/// vector window leaves the minimum graph share; sparse vector retrieval moves
/// progressively toward the maximum. Upstream commands/search.rs L335-348.
pub fn graph_result_quota(limit: usize, vector_hits: usize) -> usize {
    if limit < 2 {
        return 0;
    }
    let vector_coverage = vector_hits.min(limit) as f64 / limit as f64;
    let ratio = MAX_GRAPH_RESULT_RATIO
        - (MAX_GRAPH_RESULT_RATIO - MIN_GRAPH_RESULT_RATIO) * vector_coverage;
    ((limit as f64 * ratio).ceil() as usize).clamp(1, limit - 1)
}

/// One-hop graph expansion over ranked search results, ported from upstream
/// commands/search.rs L349-488. Adjacency comes from the prebuilt
/// RetrievalGraph (out_links ∪ in_links) instead of upstream's ad-hoc
/// alias/adjacency maps; graph-only results read page content from disk.
pub fn blend_graph_results(
    ranked_results: &mut Vec<SearchResult>,
    graph: &RetrievalGraph,
    limit: usize,
    vector_hits: usize,
    include_content: bool,
    wiki_root: &Path,
) -> usize {
    if ranked_results.is_empty() || graph.nodes.is_empty() {
        ranked_results.truncate(limit);
        return 0;
    }

    let seed_paths: Vec<String> = ranked_results
        .iter()
        .take(limit.min(MAX_GRAPH_SEEDS))
        .map(|result| graph_node_key(&result.path))
        .collect();
    let seed_set: BTreeSet<String> = seed_paths.iter().cloned().collect();
    let mut candidate_scores = BTreeMap::<String, f64>::new();
    let mut candidate_seeds = BTreeMap::<String, BTreeSet<String>>::new();
    for (rank, seed) in seed_paths.iter().enumerate() {
        let Some(seed_node) = graph.nodes.get(seed) else {
            continue;
        };
        for neighbor in seed_node.out_links.iter().chain(seed_node.in_links.iter()) {
            if seed_set.contains(neighbor) {
                continue;
            }
            *candidate_scores.entry(neighbor.clone()).or_default() += 1.0 / (rank + 1) as f64;
            candidate_seeds
                .entry(neighbor.clone())
                .or_default()
                .insert(seed_node.title.clone());
        }
    }

    let mut candidates: Vec<(String, f64)> = candidate_scores.into_iter().collect();
    candidates.sort_by(|(path_a, score_a), (path_b, score_b)| {
        score_b
            .partial_cmp(score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| path_a.cmp(path_b))
    });
    candidates.truncate(graph_result_quota(limit, vector_hits));
    if candidates.is_empty() {
        ranked_results.truncate(limit);
        return 0;
    }

    let selected_paths: BTreeSet<String> =
        candidates.iter().map(|(path, _)| path.clone()).collect();
    let mut existing = BTreeMap::<String, SearchResult>::new();
    let mut ranked_paths = Vec::new();
    for result in ranked_results.drain(..) {
        let path = graph_node_key(&result.path);
        ranked_paths.push(path.clone());
        existing.insert(path, result);
    }

    let graph_count = candidates.len();
    let base_limit = limit.saturating_sub(graph_count);
    let mut base_results: Vec<SearchResult> = ranked_paths
        .iter()
        .filter(|path| !selected_paths.contains(*path))
        .filter_map(|path| existing.get(path).cloned())
        .take(base_limit)
        .collect();

    for (path, graph_score) in candidates {
        if let Some(mut result) = existing.remove(&path) {
            result.graph_related_to = candidate_seeds
                .remove(&path)
                .unwrap_or_default()
                .into_iter()
                .collect();
            base_results.push(result);
            continue;
        }
        let Some(node) = graph.nodes.get(&path) else {
            continue;
        };
        let Ok(content) = fs::read_to_string(wiki_root.join(&node.relative_path)) else {
            continue;
        };
        let related_titles = candidate_seeds
            .remove(&path)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        let related = related_titles.join(", ");
        base_results.push(SearchResult {
            path: format!("wiki/{}", node.relative_path),
            title: node.title.clone(),
            snippet: format!("Graph neighbor of {related}"),
            title_match: false,
            score: graph_score / (RRF_K + 1.0),
            vector_score: None,
            images: extract_image_refs(&content),
            content: include_content.then_some(content),
            graph_related_to: related_titles,
        });
    }
    *ranked_results = base_results;
    graph_count
}

// SearchResult paths are project-relative ("wiki/concepts/x.md") while
// RetrievalGraph node keys are wiki-relative ("concepts/x.md").
fn graph_node_key(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .strip_prefix("wiki/")
        .unwrap_or(&normalized)
        .to_string()
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
        let stem = name.trim_end_matches(".md");
        if STRUCTURAL_IDS.contains(&stem) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let relative_path = path
            .strip_prefix(wiki_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let (title, node_type, sources) = extract_frontmatter(&content, stem);
        let links = extract_wikilinks(&content);
        raw_nodes.push((
            relative_path.clone(),
            title,
            node_type,
            relative_path,
            sources,
            links,
        ));
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
        let target = inner
            .split('|')
            .next()
            .unwrap_or(inner)
            .split('#')
            .next()
            .unwrap_or("")
            .trim();
        if !target.is_empty() {
            links.push(target.to_string());
        }
        rest = &rest[end + 2..];
    }
    links
}

fn resolve_target(raw: &str, aliases: &BTreeMap<String, String>) -> Option<String> {
    aliases.get(&normalize_target(raw)).cloned()
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

        // [[KV Cache]] resolves to concepts/kv-cache.md via stem normalization.
        assert!(
            graph.nodes["concepts/attention.md"]
                .out_links
                .contains("concepts/kv-cache.md")
        );
        assert!(
            graph.nodes["concepts/kv-cache.md"]
                .in_links
                .contains("concepts/attention.md")
        );

        let related = related_nodes(&graph, "concepts/attention.md", 5);
        assert_eq!(related[0].0, "concepts/kv-cache.md");
        // Direct link (2-way counted once each direction: 1*3.0) + shared source (1*4.0)
        // dominates the tokenizer's type-affinity-only score.
        assert!(
            related[0].1
                > related
                    .iter()
                    .find(|(id, _)| id == "concepts/tokenizer.md")
                    .map(|(_, s)| *s)
                    .unwrap_or(0.0)
        );
    }

    #[test]
    fn same_stem_pages_stay_distinct_and_anchors_resolve() {
        let temp = tempfile::tempdir().unwrap();
        let wiki = temp.path().join("wiki");
        write_page(
            &wiki,
            "concepts/attention.md",
            "---\ntype: concept\ntitle: Attention\nsources: []\n---\n\nSee [[KV Cache#layout]].\n",
        );
        write_page(
            &wiki,
            "sources/attention.md",
            "---\ntype: source\ntitle: Attention Paper\nsources: []\n---\n\nSource notes.\n",
        );
        write_page(
            &wiki,
            "concepts/kv-cache.md",
            "---\ntype: concept\ntitle: KV Cache\nsources: []\n---\n\nCaches keys and values.\n",
        );
        write_page(&wiki, "index.md", "# Index\n\n- [[Attention]]\n");

        let graph = build_retrieval_graph(&wiki);

        // Same-stem pages are distinct nodes; structural index.md is skipped.
        assert!(graph.nodes.contains_key("concepts/attention.md"));
        assert!(graph.nodes.contains_key("sources/attention.md"));
        assert!(!graph.nodes.contains_key("index.md"));
        assert!(!graph.nodes.contains_key("index"));

        // [[KV Cache#layout]] resolves after anchor stripping.
        assert!(
            graph.nodes["concepts/attention.md"]
                .out_links
                .contains("concepts/kv-cache.md")
        );
    }

    #[test]
    fn type_affinity_defaults_to_half_for_unknown_pairs() {
        assert_eq!(type_affinity("concept", "synthesis"), 1.2);
        assert_eq!(type_affinity("source", "source"), 0.5);
        assert_eq!(type_affinity("other", "concept"), 0.5);
    }

    fn keyword_result(path: &str, title: &str, score: f64) -> SearchResult {
        SearchResult {
            path: path.to_string(),
            title: title.to_string(),
            snippet: String::new(),
            title_match: false,
            score,
            vector_score: None,
            images: Vec::new(),
            content: None,
            graph_related_to: Vec::new(),
        }
    }

    #[test]
    fn graph_result_quota_tracks_vector_coverage() {
        // Full vector coverage → 15% floor; ceil(10 * 0.15) = 2.
        assert_eq!(graph_result_quota(10, 10), 2);
        // No vector hits → 30%; ceil(10 * 0.30) = 3.
        assert_eq!(graph_result_quota(10, 0), 3);
        assert_eq!(graph_result_quota(1, 0), 0);
        assert_eq!(graph_result_quota(0, 0), 0);
        // Clamped to limit - 1.
        assert_eq!(graph_result_quota(2, 0), 1);
    }

    #[test]
    fn blend_injects_one_hop_neighbors_with_synthetic_scores() {
        let temp = tempfile::tempdir().unwrap();
        let wiki = temp.path().join("wiki");
        write_page(
            &wiki,
            "concepts/attention.md",
            "---\ntitle: Attention\n---\n\nUses [[KV Cache]] and [[Softmax]].\n",
        );
        write_page(&wiki, "concepts/kv-cache.md", "---\ntitle: KV Cache\n---\n\nCache.\n");
        write_page(&wiki, "concepts/softmax.md", "---\ntitle: Softmax\n---\n\nSoftmax.\n");

        let graph = build_retrieval_graph(&wiki);
        let mut results = vec![keyword_result("wiki/concepts/attention.md", "Attention", 5.0)];
        let graph_hits = blend_graph_results(&mut results, &graph, 3, 0, false, &wiki);

        assert_eq!(graph_hits, 1); // quota for limit=3, vector=0 is ceil(0.9)=1
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "wiki/concepts/attention.md");
        let neighbor = &results[1];
        assert_eq!(neighbor.path, "wiki/concepts/kv-cache.md");
        assert_eq!(neighbor.snippet, "Graph neighbor of Attention");
        assert_eq!(neighbor.graph_related_to, vec!["Attention".to_string()]);
        // Seed rank 0 contributes 1.0/(0+1); synthetic score = 1.0 / (RRF_K + 1.0).
        assert!((neighbor.score - 1.0 / 61.0).abs() < 1e-9);
        assert!(neighbor.content.is_none());
    }

    #[test]
    fn blend_promotes_existing_results_in_place_without_duplicates() {
        let temp = tempfile::tempdir().unwrap();
        let wiki = temp.path().join("wiki");
        write_page(&wiki, "concepts/a.md", "---\ntitle: Alpha\n---\n\nLinks [[Delta]].\n");
        write_page(&wiki, "concepts/b.md", "---\ntitle: Beta\n---\n\nBeta.\n");
        write_page(&wiki, "concepts/c.md", "---\ntitle: Gamma\n---\n\nGamma.\n");
        write_page(&wiki, "concepts/delta.md", "---\ntitle: Delta\n---\n\nDelta.\n");

        let graph = build_retrieval_graph(&wiki);
        // Delta sits below the seed window (seeds = first limit.min(20) = 3),
        // so it becomes a graph candidate that must be promoted, not duplicated.
        let mut results = vec![
            keyword_result("wiki/concepts/a.md", "Alpha", 5.0),
            keyword_result("wiki/concepts/b.md", "Beta", 4.0),
            keyword_result("wiki/concepts/c.md", "Gamma", 3.0),
            keyword_result("wiki/concepts/delta.md", "Delta", 2.0),
        ];
        let graph_hits = blend_graph_results(&mut results, &graph, 3, 0, false, &wiki);

        assert_eq!(graph_hits, 1); // quota(3, 0) = 1
        let paths: Vec<&str> = results.iter().map(|result| result.path.as_str()).collect();
        // base_limit = 2 keeps Alpha/Beta; Gamma is displaced; Delta promoted.
        assert_eq!(
            paths,
            vec!["wiki/concepts/a.md", "wiki/concepts/b.md", "wiki/concepts/delta.md"],
        );
        let unique: BTreeSet<&str> = paths.iter().copied().collect();
        assert_eq!(unique.len(), paths.len());
        assert_eq!(results[2].graph_related_to, vec!["Alpha".to_string()]);
        // Promoted result keeps its keyword score (not the synthetic graph score).
        assert_eq!(results[2].score, 2.0);
    }

    #[test]
    fn blend_returns_zero_for_empty_results_or_graph() {
        let temp = tempfile::tempdir().unwrap();
        let wiki = temp.path().join("wiki");
        write_page(&wiki, "concepts/a.md", "---\ntitle: Alpha\n---\n\nSolo.\n");
        let graph = build_retrieval_graph(&wiki);

        let mut empty_results: Vec<SearchResult> = Vec::new();
        assert_eq!(blend_graph_results(&mut empty_results, &graph, 5, 0, false, &wiki), 0);

        let empty_graph = RetrievalGraph::default();
        let mut results = vec![keyword_result("wiki/concepts/a.md", "Alpha", 5.0)];
        assert_eq!(blend_graph_results(&mut results, &empty_graph, 5, 0, false, &wiki), 0);
        assert_eq!(results.len(), 1);
    }
}
