use std::collections::HashSet;

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

/// Fetch a URL and extract readable markdown.
// wired up in Task 10
#[allow(dead_code)]
pub async fn fetch_url(client: &reqwest::Client, url: &str) -> Result<ExtractedPage, String> {
    let response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (compatible; KnowledgeCanvas/1.0)")
        .send()
        .await
        .map_err(|e| format!("fetch failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("fetch failed: HTTP {}", response.status()));
    }
    let html = response.text().await.map_err(|e| format!("read body failed: {e}"))?;
    Ok(html_to_markdown(&html))
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
}
