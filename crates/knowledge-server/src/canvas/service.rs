use crate::canvas::document::CanvasNode;

#[derive(Debug, Clone)]
pub struct SearchResultEntry {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Format one upstream *text* node into a reference block. Returns `None` for KB
/// (handled via async RAG) and whenever the node has no usable text.
pub fn format_text_reference_block(node: &CanvasNode) -> Option<String> {
    match node.r#type.as_str() {
        "note" => node
            .data
            .get("markdown")
            .and_then(|v| v.as_str())
            .filter(|m| !m.trim().is_empty())
            .map(|m| format!("Note:\n{m}")),
        "url" => {
            let md = node.data.get("markdown").and_then(|v| v.as_str()).unwrap_or("");
            if md.trim().is_empty() {
                return None;
            }
            let title = node.data.get("title").and_then(|v| v.as_str()).unwrap_or("");
            Some(format!("Web page: {title}\n{md}"))
        }
        "ai_analyze" => crate::canvas::document::active_version_content(&node.data)
            .filter(|c| !c.trim().is_empty())
            .map(|c| format!("Prior analysis:\n{c}")),
        "search" => node
            .data
            .get("markdown")
            .and_then(|v| v.as_str())
            .filter(|m| !m.trim().is_empty())
            .map(|m| format!("Search results:\n{m}")),
        // Text-only reference: the downstream LLM sees the prompt + asset URL, not
        // the pixels (no vision/multimodal — YAGNI per spec §8).
        "ai_image" => {
            let url = node.data.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if url.trim().is_empty() {
                return None;
            }
            let prompt = node.data.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
            Some(format!("Generated image (prompt: {prompt}): {url}"))
        }
        _ => None,
    }
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

/// Build the (system, user) prompt pair that synthesises a single web-search
/// query from the node's guidance text and its upstream reference blocks.
pub fn build_search_query_prompt(guidance: &str, blocks: &[String]) -> (String, String) {
    let system =
        "Output exactly one concise web search query and nothing else. Do not explain, quote, or add punctuation beyond the query itself."
            .to_string();
    let mut user = format!("Guidance: {guidance}\n\nSources:\n");
    for (i, b) in blocks.iter().enumerate() {
        user.push_str(&format!("--- Source {} ---\n{}\n\n", i + 1, b));
    }
    (system, user)
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

    fn tnode(ty: &str, data: serde_json::Value) -> CanvasNode {
        CanvasNode { id: "n".into(), r#type: ty.into(), x: 0.0, y: 0.0, w: 280.0, h: 160.0, data }
    }

    fn edge(id: &str, source: &str, target: &str) -> CanvasEdge {
        CanvasEdge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            ..Default::default()
        }
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

    #[test]
    fn format_text_reference_block_covers_each_text_type() {
        assert_eq!(
            format_text_reference_block(&tnode("note", serde_json::json!({ "markdown": "hi" }))),
            Some("Note:\nhi".to_string())
        );
        assert_eq!(
            format_text_reference_block(&tnode("url", serde_json::json!({ "title": "T", "markdown": "body" }))),
            Some("Web page: T\nbody".to_string())
        );
        let analysis = serde_json::json!({
            "versions": [{ "id": "v1", "content": "result" }], "activeVersionId": "v1"
        });
        assert_eq!(
            format_text_reference_block(&tnode("ai_analyze", analysis)),
            Some("Prior analysis:\nresult".to_string())
        );
        assert_eq!(
            format_text_reference_block(&tnode("search", serde_json::json!({ "markdown": "res md" }))),
            Some("Search results:\nres md".to_string())
        );
        assert_eq!(
            format_text_reference_block(&tnode(
                "ai_image",
                serde_json::json!({ "prompt": "a fox", "url": "/api/assets/x" })
            )),
            Some("Generated image (prompt: a fox): /api/assets/x".to_string())
        );
    }

    #[test]
    fn format_text_reference_block_skips_empty_and_kb() {
        assert_eq!(format_text_reference_block(&tnode("note", serde_json::json!({ "markdown": "  " }))), None);
        assert_eq!(format_text_reference_block(&tnode("search", serde_json::json!({}))), None);
        assert_eq!(format_text_reference_block(&tnode("kb", serde_json::json!({ "projectId": "p" }))), None);
    }

    #[test]
    fn build_search_query_prompt_embeds_guidance_and_sources() {
        let (system, user) = build_search_query_prompt("find recent news", &["block A".into(), "block B".into()]);
        assert!(system.to_lowercase().contains("search query"));
        assert!(user.contains("find recent news"));
        assert!(user.contains("block A"));
        assert!(user.contains("block B"));
    }
}
