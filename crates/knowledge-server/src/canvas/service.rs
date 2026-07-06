use crate::canvas::document::CanvasDocument;

#[derive(Debug, Clone)]
pub struct SearchResultEntry {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Text blocks from incoming Note/URL/prior-analysis nodes (KB handled
/// separately via RAG).
pub fn collect_reference_blocks(doc: &CanvasDocument, node_id: &str) -> Vec<String> {
    let sources = doc.incoming_source_ids(node_id);
    let mut blocks = Vec::new();
    for src in sources {
        let Some(node) = doc.node(&src) else { continue };
        match node.r#type.as_str() {
            "note" => {
                if let Some(md) = node.data.get("markdown").and_then(|v| v.as_str())
                    && !md.trim().is_empty()
                {
                    blocks.push(format!("Note:\n{md}"));
                }
            }
            "url" => {
                let title = node.data.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let md = node.data.get("markdown").and_then(|v| v.as_str()).unwrap_or("");
                if !md.trim().is_empty() {
                    blocks.push(format!("Web page: {title}\n{md}"));
                }
            }
            "ai_analyze" => {
                if let Some(content) = crate::canvas::document::active_version_content(&node.data) {
                    blocks.push(format!("Prior analysis:\n{content}"));
                }
            }
            _ => {}
        }
    }
    blocks
}

/// Project ids of incoming KB nodes, for RAG retrieval at run time.
pub fn referenced_kb_project_ids(doc: &CanvasDocument, node_id: &str) -> Vec<String> {
    doc.incoming_source_ids(node_id)
        .into_iter()
        .filter_map(|src| doc.node(&src).cloned())
        .filter(|n| n.r#type == "kb")
        .filter_map(|n| n.data.get("projectId").and_then(|v| v.as_str()).map(String::from))
        .collect()
}

pub fn search_results_to_markdown(query: &str, results: &[SearchResultEntry]) -> String {
    let mut out = format!("Search results for \"{query}\":\n\n");
    for r in results {
        out.push_str(&format!("- [{}]({})\n  {}\n", r.title, r.url, r.snippet));
    }
    out.trim_end().to_string()
}

pub fn build_analyze_prompt(node_prompt: &str, blocks: &[String]) -> String {
    let mut prompt = String::new();
    if !blocks.is_empty() {
        prompt.push_str("Use the following referenced sources to answer.\n\n");
        for (i, b) in blocks.iter().enumerate() {
            prompt.push_str(&format!("--- Source {} ---\n{}\n\n", i + 1, b));
        }
    }
    prompt.push_str("Task:\n");
    prompt.push_str(node_prompt);
    prompt
}

#[cfg(test)]
mod context_tests {
    use super::*;
    use crate::canvas::document::{CanvasEdge, CanvasNode, Viewport};

    fn node(id: &str, ty: &str, data: serde_json::Value) -> CanvasNode {
        CanvasNode {
            id: id.to_string(),
            r#type: ty.to_string(),
            x: 0.0,
            y: 0.0,
            w: 280.0,
            h: 160.0,
            data,
        }
    }

    fn edge(id: &str, source: &str, target: &str) -> CanvasEdge {
        CanvasEdge { id: id.to_string(), source: source.to_string(), target: target.to_string() }
    }

    #[test]
    fn collect_reference_blocks_includes_note_and_url_text() {
        let doc = CanvasDocument {
            nodes: vec![
                node("n1", "note", serde_json::json!({ "markdown": "a note body" })),
                node(
                    "u1",
                    "url",
                    serde_json::json!({ "title": "Example", "markdown": "page body" }),
                ),
                node("t", "ai_analyze", serde_json::json!({})),
            ],
            edges: vec![edge("e1", "n1", "t"), edge("e2", "u1", "t")],
            viewport: Viewport::default(),
        };
        let blocks = collect_reference_blocks(&doc, "t");
        assert_eq!(blocks.len(), 2);
        assert!(blocks.iter().any(|b| b.contains("Note:") && b.contains("a note body")));
        assert!(blocks
            .iter()
            .any(|b| b.contains("Web page: Example") && b.contains("page body")));
    }

    #[test]
    fn collect_reference_blocks_uses_active_version_of_prior_analysis() {
        let analysis = serde_json::json!({
            "versions": [{ "id": "v1", "content": "prior result" }],
            "activeVersionId": "v1"
        });
        let doc = CanvasDocument {
            nodes: vec![
                node("a1", "ai_analyze", analysis),
                node("t", "ai_analyze", serde_json::json!({})),
            ],
            edges: vec![edge("e1", "a1", "t")],
            viewport: Viewport::default(),
        };
        let blocks = collect_reference_blocks(&doc, "t");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("Prior analysis:") && blocks[0].contains("prior result"));
    }

    #[test]
    fn referenced_kb_project_ids_collects_incoming_kb_projects() {
        let doc = CanvasDocument {
            nodes: vec![
                node("k1", "kb", serde_json::json!({ "projectId": "proj-1" })),
                node("n1", "note", serde_json::json!({ "markdown": "x" })),
                node("t", "ai_analyze", serde_json::json!({})),
            ],
            edges: vec![edge("e1", "k1", "t"), edge("e2", "n1", "t")],
            viewport: Viewport::default(),
        };
        let ids = referenced_kb_project_ids(&doc, "t");
        assert_eq!(ids, vec!["proj-1".to_string()]);
    }

    #[test]
    fn search_results_to_markdown_formats_entries() {
        let results = vec![
            SearchResultEntry {
                title: "First".into(),
                url: "https://a.test".into(),
                snippet: "snippet one".into(),
            },
            SearchResultEntry {
                title: "Second".into(),
                url: "https://b.test".into(),
                snippet: "snippet two".into(),
            },
        ];
        let md = search_results_to_markdown("cats", &results);
        assert!(md.starts_with("Search results for \"cats\":"));
        assert!(md.contains("- [First](https://a.test)"));
        assert!(md.contains("snippet two"));
    }

    #[test]
    fn build_analyze_prompt_combines_prompt_and_blocks() {
        let blocks = vec!["block A".to_string(), "block B".to_string()];
        let prompt = build_analyze_prompt("summarize", &blocks);
        assert!(prompt.contains("--- Source 1 ---\nblock A"));
        assert!(prompt.contains("--- Source 2 ---\nblock B"));
        assert!(prompt.contains("Task:\nsummarize"));
    }

    #[test]
    fn build_analyze_prompt_omits_sources_when_empty() {
        let prompt = build_analyze_prompt("just do it", &[]);
        assert!(!prompt.contains("referenced sources"));
        assert!(prompt.contains("Task:\njust do it"));
    }
}
