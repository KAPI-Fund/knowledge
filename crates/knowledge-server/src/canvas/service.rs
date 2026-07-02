use std::collections::HashSet;
use std::net::IpAddr;

use scraper::{Html, Selector};

/// Reference: upstream_llm_wiki/extension/Readability.js (content extraction)
/// + upstream_llm_wiki/extension/Turndown.js (HTML -> markdown).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedPage {
    pub title: String,
    pub markdown: String,
}

pub fn html_to_markdown(html: &str) -> ExtractedPage {
    let doc = Html::parse_document(html);

    let title = doc
        .select(&Selector::parse("title").unwrap())
        .next()
        .map(|t| t.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Untitled".to_string());

    // Prefer <article>/<main> as the content root; fall back to <body>.
    let root_html = ["article", "main", "body"]
        .iter()
        .find_map(|sel| {
            doc.select(&Selector::parse(sel).unwrap())
                .next()
                .map(|el| el.html())
        })
        .unwrap_or_else(|| html.to_string());

    let root = Html::parse_fragment(&root_html);

    // Collect NodeIds that belong to skip subtrees (nav, footer, script, style, aside).
    // Use HashSet with inferred NodeId type so we don't need to name ego_tree directly.
    let skip_sel = Selector::parse("nav, footer, script, style, aside").unwrap();
    let mut skip_ids = HashSet::new();
    for skip_el in root.select(&skip_sel) {
        skip_ids.insert(skip_el.id());
        for descendant in skip_el.descendants() {
            skip_ids.insert(descendant.id());
        }
    }

    let block_sel = Selector::parse("h1, h2, h3, h4, p, li, pre").unwrap();
    let mut out = String::new();

    for el in root.select(&block_sel) {
        if skip_ids.contains(&el.id()) {
            continue;
        }
        if el.ancestors().any(|a| skip_ids.contains(&a.id())) {
            continue;
        }
        let text = inline_markdown(el);
        if text.trim().is_empty() {
            continue;
        }
        let name = el.value().name();
        match name {
            "h1" => out.push_str(&format!("# {text}\n\n")),
            "h2" => out.push_str(&format!("## {text}\n\n")),
            "h3" => out.push_str(&format!("### {text}\n\n")),
            "h4" => out.push_str(&format!("#### {text}\n\n")),
            "li" => out.push_str(&format!("- {text}\n")),
            "pre" => out.push_str(&format!("```\n{text}\n```\n\n")),
            _ => out.push_str(&format!("{text}\n\n")),
        }
    }

    ExtractedPage { title, markdown: out.trim().to_string() }
}

fn inline_markdown(el: scraper::ElementRef) -> String {
    let mut s = String::new();
    for child in el.children() {
        if let Some(text) = child.value().as_text() {
            s.push_str(text);
        } else if let Some(child_el) = scraper::ElementRef::wrap(child) {
            if child_el.value().name() == "a" {
                let href = child_el.value().attr("href").unwrap_or("");
                let label = child_el.text().collect::<String>();
                if href.is_empty() {
                    s.push_str(&label);
                } else {
                    s.push_str(&format!("[{label}]({href})"));
                }
            } else {
                s.push_str(&inline_markdown(child_el));
            }
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cap on the response body we will buffer from an extracted page (5 MiB).
pub const MAX_EXTRACT_BYTES: usize = 5 * 1024 * 1024;

/// Guard against SSRF: only allow http/https to public hosts. A literal-IP
/// host in a private/loopback/link-local/unspecified range is rejected, as is
/// `localhost`. This blocks the direct vectors (cloud metadata, internal
/// services). It does NOT defend against a public hostname that resolves to a
/// private IP (DNS rebinding) — that needs a pinning connector.
pub fn validate_public_url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|_| "invalid URL".to_string())?;
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(format!("unsupported URL scheme: {other}")),
    }
    let host = url.host_str().ok_or_else(|| "URL has no host".to_string())?;
    if host.eq_ignore_ascii_case("localhost") {
        return Err("URL host is not allowed".to_string());
    }
    // IPv6 literals arrive bracketed (e.g. "[::1]"); strip them before parsing.
    let host_ip = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    if let Ok(ip) = host_ip.parse::<IpAddr>()
        && is_blocked_ip(&ip)
    {
        return Err("URL host is not allowed".to_string());
    }
    Ok(url)
}

fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                // 100.64.0.0/10 shared address space (CGNAT).
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 0x40)
        }
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_blocked_ip(&IpAddr::V4(mapped));
            }
            v6.is_loopback()
                || v6.is_unspecified()
                // Unique local fc00::/7.
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                // Link-local fe80::/10.
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Fetch a URL and extract readable markdown.
pub async fn fetch_url(client: &reqwest::Client, url: &str) -> Result<ExtractedPage, String> {
    let url = validate_public_url(url)?;
    let mut response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (compatible; KnowledgeCanvas/1.0)")
        .send()
        .await
        .map_err(|e| format!("fetch failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("fetch failed: HTTP {}", response.status()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| format!("read body failed: {e}"))? {
        if body.len() + chunk.len() > MAX_EXTRACT_BYTES {
            return Err("response too large".to_string());
        }
        body.extend_from_slice(&chunk);
    }
    let html = String::from_utf8_lossy(&body);
    Ok(html_to_markdown(&html))
}

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
mod url_tests {
    use super::*;

    #[test]
    fn html_to_markdown_extracts_title_and_text() {
        let html = r#"
            <html><head><title>Sample Page</title></head>
            <body>
              <nav>ignore me</nav>
              <article>
                <h1>Big Heading</h1>
                <p>First paragraph with a <a href="https://x.test">link</a>.</p>
                <h2>Sub</h2>
                <p>Second paragraph.</p>
              </article>
              <footer>footer junk</footer>
            </body></html>
        "#;
        let extracted = html_to_markdown(html);
        assert_eq!(extracted.title, "Sample Page");
        assert!(extracted.markdown.contains("# Big Heading"));
        assert!(extracted.markdown.contains("## Sub"));
        assert!(extracted.markdown.contains("First paragraph with a [link](https://x.test)."));
        assert!(extracted.markdown.contains("Second paragraph."));
        assert!(!extracted.markdown.contains("ignore me"));
        assert!(!extracted.markdown.contains("footer junk"));
    }

    #[test]
    fn html_to_markdown_falls_back_to_untitled() {
        let extracted = html_to_markdown("<html><body><p>hi</p></body></html>");
        assert_eq!(extracted.title, "Untitled");
        assert!(extracted.markdown.contains("hi"));
    }

    #[test]
    fn validate_public_url_accepts_public_http_and_https() {
        assert!(validate_public_url("http://93.184.216.34/").is_ok());
        assert!(validate_public_url("https://example.com/page").is_ok());
    }

    #[test]
    fn validate_public_url_rejects_non_http_schemes() {
        assert!(validate_public_url("file:///etc/passwd").is_err());
        assert!(validate_public_url("ftp://example.com/x").is_err());
        assert!(validate_public_url("gopher://example.com/").is_err());
    }

    #[test]
    fn validate_public_url_rejects_localhost_and_loopback() {
        assert!(validate_public_url("http://localhost/").is_err());
        assert!(validate_public_url("http://127.0.0.1/").is_err());
        assert!(validate_public_url("http://[::1]/").is_err());
    }

    #[test]
    fn validate_public_url_rejects_private_and_link_local() {
        assert!(validate_public_url("http://10.0.0.1/").is_err());
        assert!(validate_public_url("http://192.168.1.1/").is_err());
        assert!(validate_public_url("http://172.16.5.4/").is_err());
        // Cloud metadata endpoint (link-local).
        assert!(validate_public_url("http://169.254.169.254/latest/meta-data/").is_err());
    }

    #[test]
    fn validate_public_url_rejects_unspecified_and_mapped() {
        assert!(validate_public_url("http://0.0.0.0/").is_err());
        // IPv4-mapped IPv6 form of a loopback address.
        assert!(validate_public_url("http://[::ffff:127.0.0.1]/").is_err());
    }
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
